use std::path::{Path, PathBuf};

/// 内置提示音（随二进制打包，便携版不带 sound 文件夹）。
/// 外部 `sound/` 里的同名 wav 优先覆盖，用于用户自定义。
pub const EMBEDDED: &[(&str, &[u8])] = &[
    (
        "notify.wav",
        include_bytes!("../resources/sound/notify.wav"),
    ),
    (
        "notify_plat.wav",
        include_bytes!("../resources/sound/notify_plat.wav"),
    ),
    (
        "notify_rare.wav",
        include_bytes!("../resources/sound/notify_rare.wav"),
    ),
];

/// 按名字找内置音源；名字为空或不匹配时回落第一个。
fn pick_embedded(requested: &str) -> Option<&'static [u8]> {
    let requested_name = Path::new(requested)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let mut first = None;
    for (name, bytes) in EMBEDDED {
        if first.is_none() {
            first = Some(*bytes);
        }
        if !requested_name.is_empty() && name.eq_ignore_ascii_case(requested_name) {
            return Some(*bytes);
        }
    }
    first
}

pub fn list_wavs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
        })
        .collect();
    files.sort_by_key(|path| path.file_name().map(|name| name.to_ascii_lowercase()));
    files
}

pub fn search_dirs(
    exe_dir: Option<&Path>,
    resource_dir: Option<&Path>,
    dev_dir: Option<&Path>,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = exe_dir {
        dirs.push(dir.join("sound"));
    }
    if let Some(dir) = resource_dir {
        dirs.push(dir.join("sound"));
    }
    if let Some(dir) = dev_dir {
        dirs.push(dir.join("sound"));
    }
    dirs
}

pub fn pick_sound(dirs: &[PathBuf], requested: &str) -> Option<PathBuf> {
    let requested_name = Path::new(requested)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let mut first = None;
    for dir in dirs {
        for wav in list_wavs(dir) {
            if first.is_none() {
                first = Some(wav.clone());
            }
            if !requested_name.is_empty()
                && wav
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case(requested_name))
            {
                return Some(wav);
            }
        }
    }
    first
}

#[cfg(windows)]
pub fn play(path: Option<&Path>) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT};
    use windows::Win32::System::Diagnostics::Debug::MessageBeep;
    use windows::Win32::UI::WindowsAndMessaging::MB_ICONEXCLAMATION;
    use windows::core::PCWSTR;

    let Some(path) = path else {
        // SAFETY: MessageBeep has no pointer arguments.
        unsafe {
            let _ = MessageBeep(MB_ICONEXCLAMATION);
        }
        return false;
    };
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    // SAFETY: `wide` is a NUL-terminated UTF-16 path that stays alive for the call.
    unsafe {
        PlaySoundW(
            PCWSTR(wide.as_ptr()),
            None,
            SND_FILENAME | SND_ASYNC | SND_NODEFAULT,
        )
        .as_bool()
    }
}

/// 从内存播放内置 wav（SND_MEMORY）；`'static` 内嵌数据满足异步播放期有效的约束。
#[cfg(windows)]
pub fn play_bytes(bytes: &'static [u8]) -> bool {
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    use windows::core::PCWSTR;
    // SAFETY: SND_MEMORY 下 PCWSTR 只是指向 wav 数据的指针；数据为 'static 内嵌常量。
    unsafe {
        PlaySoundW(
            PCWSTR(bytes.as_ptr().cast()),
            None,
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        )
        .as_bool()
    }
}

/// 播放提示音：外部 sound/ 同名文件优先，其次内置，都没有才用系统提示音。
pub fn play_requested(dirs: &[PathBuf], requested: &str) -> bool {
    if let Some(path) = pick_sound(dirs, requested) {
        return play(Some(&path));
    }
    if let Some(bytes) = pick_embedded(requested) {
        return play_bytes(bytes);
    }
    play(None)
}
