//! 被动监听：只读 Input Report 的设备由常驻线程收帧，刷新直接读缓存。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::hid;
use super::profiles::{self, Profile, Transport};
use super::{BatteryState, DeviceSpec, ReadResult};

const MAX_LISTENERS: usize = 32;
const READ_SLICE_MS: u32 = 500;
const REOPEN_AFTER_S: u64 = 5;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ListenerKey {
    path: String,
    profile_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CacheState {
    Waiting,
    Heard,
    Gone,
}

struct Cache {
    state: CacheState,
    level: Option<u8>,
    charging: bool,
    seen_at: Option<Instant>,
    first_frame_at: Option<Instant>,
    waited: bool,
}

struct Listener {
    cache: Arc<(Mutex<Cache>, Condvar)>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

/// 进程级被动监听管理器；由 `AppState` 持有。
pub struct Hub {
    inner: Mutex<HashMap<ListenerKey, Listener>>,
}

impl Default for Hub {
    fn default() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }
}

impl Hub {
    pub fn new() -> Self {
        Self::default()
    }

    /// 按当前配置差异启动/停止监听；每轮刷新调用一次。
    pub fn sync(&self, devices: &[DeviceSpec], table: &HashMap<String, Profile>) {
        let mut desired: HashMap<ListenerKey, (DeviceSpec, Profile)> = HashMap::new();
        for dev in devices {
            let Some(profile) = profiles::for_device(dev, table) else {
                continue;
            };
            if profile.request.transport != Transport::Passive {
                continue;
            }
            let Some((path, _)) = hid::passive_path(dev, profile) else {
                continue;
            };
            desired.insert(
                ListenerKey {
                    path,
                    profile_id: profile.id.clone(),
                },
                (dev.clone(), profile.clone()),
            );
        }

        let Ok(mut listeners) = self.inner.lock() else {
            return;
        };
        let stale: Vec<ListenerKey> = listeners
            .keys()
            .filter(|key| !desired.contains_key(*key))
            .cloned()
            .collect();
        for key in stale {
            if let Some(mut listener) = listeners.remove(&key) {
                listener.stop.store(true, Ordering::Relaxed);
                if let Some(worker) = listener.worker.take() {
                    let _ = worker.join();
                }
            }
        }

        for (key, (dev, profile)) in desired {
            if listeners.contains_key(&key) {
                continue;
            }
            if listeners.len() >= MAX_LISTENERS {
                eprintln!("[passive] 超过 {MAX_LISTENERS} 个监听，跳过 {}", key.path);
                continue;
            }
            listeners.insert(key.clone(), spawn_listener(key, dev, profile));
        }
    }

    pub fn read(&self, dev: &DeviceSpec, profile: &Profile) -> ReadResult {
        let Some((path, _)) = hid::passive_path(dev, profile) else {
            return absent();
        };
        let key = ListenerKey {
            path,
            profile_id: profile.id.clone(),
        };
        let Ok(listeners) = self.inner.lock() else {
            return absent();
        };
        let Some(listener) = listeners.get(&key) else {
            return absent();
        };
        let (lock, signal) = &*listener.cache;
        let Ok(mut cache) = lock.lock() else {
            return absent();
        };

        if cache.first_frame_at.is_none() && !cache.waited {
            let wait_ms = profile
                .passive
                .map(|passive| passive.initial_wait_ms)
                .unwrap_or(2500);
            let (guard, _) = signal
                .wait_timeout(cache, Duration::from_millis(wait_ms))
                .unwrap_or_else(|poison| poison.into_inner());
            cache = guard;
            cache.waited = true;
        }

        match cache.state {
            CacheState::Heard => {
                let stale_after_s = profile
                    .passive
                    .map(|passive| passive.stale_after_s)
                    .unwrap_or(15.0);
                let fresh = cache.seen_at.is_some_and(|seen| {
                    stale_after_s == 0.0 || seen.elapsed().as_secs_f64() <= stale_after_s
                });
                if fresh {
                    ReadResult {
                        state: BatteryState::Ok,
                        level: cache.level,
                        charging: cache.charging,
                    }
                } else {
                    ReadResult {
                        state: BatteryState::Sleep,
                        level: None,
                        charging: cache.charging,
                    }
                }
            }
            CacheState::Gone if cache.seen_at.is_some() => ReadResult {
                state: BatteryState::Sleep,
                level: None,
                charging: cache.charging,
            },
            CacheState::Waiting => ReadResult {
                state: BatteryState::Sleep,
                level: None,
                charging: false,
            },
            CacheState::Gone => absent(),
        }
    }

    pub fn shutdown(&self) {
        let Ok(mut listeners) = self.inner.lock() else {
            return;
        };
        for (_, mut listener) in listeners.drain() {
            listener.stop.store(true, Ordering::Relaxed);
            if let Some(worker) = listener.worker.take() {
                let _ = worker.join();
            }
        }
    }
}

fn absent() -> ReadResult {
    ReadResult {
        state: BatteryState::Absent,
        level: None,
        charging: false,
    }
}

fn spawn_listener(key: ListenerKey, dev: DeviceSpec, profile: Profile) -> Listener {
    let cache = Arc::new((
        Mutex::new(Cache {
            state: CacheState::Waiting,
            level: None,
            charging: false,
            seen_at: None,
            first_frame_at: None,
            waited: false,
        }),
        Condvar::new(),
    ));
    let stop = Arc::new(AtomicBool::new(false));
    let worker_cache = Arc::clone(&cache);
    let worker_stop = Arc::clone(&stop);
    let worker = thread::Builder::new()
        .name("vxe-passive".to_string())
        .spawn(move || run_listener(key, dev, profile, worker_cache, worker_stop))
        .ok();
    Listener {
        cache,
        stop,
        worker,
    }
}

fn run_listener(
    key: ListenerKey,
    dev: DeviceSpec,
    profile: Profile,
    cache: Arc<(Mutex<Cache>, Condvar)>,
    stop: Arc<AtomicBool>,
) {
    let mut path = key.path;
    let mut in_len = profile.request.in_len.unwrap_or(64);
    let mut reader: Option<hid::PassiveReader> = None;
    let mut last_frame = Instant::now();
    let mut backoff = Duration::from_secs(1);

    loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        if reader.is_none() {
            if let Some((new_path, new_len)) = hid::passive_path(&dev, &profile) {
                path = new_path;
                in_len = new_len;
            }
            reader = hid::PassiveReader::open(&path);
            if reader.is_none() {
                mark_gone(&cache);
                if !sleep_stop(&stop, backoff) {
                    return;
                }
                backoff = (backoff * 2).min(Duration::from_secs(10));
                continue;
            }
            backoff = Duration::from_secs(1);
        }

        let Some(active) = reader.as_ref() else {
            continue;
        };
        if let Some((level, charging)) =
            active.read(usize::from(in_len).min(1024), READ_SLICE_MS, &profile)
        {
            let (lock, signal) = &*cache;
            if let Ok(mut cache) = lock.lock() {
                let now = Instant::now();
                cache.state = CacheState::Heard;
                cache.level = Some(level);
                cache.charging = charging;
                cache.seen_at = Some(now);
                if cache.first_frame_at.is_none() {
                    cache.first_frame_at = Some(now);
                }
                signal.notify_all();
            }
            last_frame = Instant::now();
        } else if last_frame.elapsed() >= Duration::from_secs(REOPEN_AFTER_S) {
            reader = None;
            last_frame = Instant::now();
        }
    }
}

fn mark_gone(cache: &Arc<(Mutex<Cache>, Condvar)>) {
    let (lock, signal) = &**cache;
    if let Ok(mut cache) = lock.lock() {
        cache.state = CacheState::Gone;
        signal.notify_all();
    }
}

fn sleep_stop(stop: &AtomicBool, duration: Duration) -> bool {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
    true
}
