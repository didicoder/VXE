//! 设备型号目录：描述“这是哪台设备”，与“怎么读”的协议库分开。
//!
//! 每个型号一条独立信息（名称、厂商、图标、VID/PID、型号码、默认协议、
//! 产品名匹配串）。多个型号可以共用同一份读取协议；VID:PID 被共用时，
//! 扫描会返回多个候选，由添加窗让用户选择。
//!
//! 没有显式条目的协议会自动生成一条同 id 的型号（名称取协议名），
//! 保证旧协议始终有可展示的设备身份；常用共用型号在 `MODELS_JSON` 里拆开。

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::profiles::{KnownId, Profile};

/// 一个可选择的设备型号。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    #[serde(default)]
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub link: String,
    /// 默认读取协议（profile id）。
    pub profile: String,
    #[serde(default)]
    pub known_ids: Vec<KnownId>,
    #[serde(default)]
    pub model_code: Option<u16>,
    /// HID 产品串匹配片段（不区分大小写）；空 = 不参与产品名匹配。
    #[serde(default)]
    pub products: Vec<String>,
    #[serde(default)]
    pub keywords: String,
}

/// 添加窗下拉用的轻量选项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelChoice {
    pub id: String,
    pub label: String,
    pub vendor: String,
    pub keywords: String,
    pub profile: String,
    pub icon: String,
    pub link: String,
}

/// 一次型号识别的结果。`selected` 为空且候选非空 = 需要用户选择。
#[derive(Debug, Clone)]
pub struct ModelPick<'a> {
    pub candidates: Vec<&'a Model>,
    pub selected: Option<&'a Model>,
}

impl ModelPick<'_> {
    #[must_use]
    pub fn ambiguous(&self) -> bool {
        self.selected.is_none() && self.candidates.len() > 1
    }
}

pub fn choice(model: &Model) -> ModelChoice {
    ModelChoice {
        id: model.id.clone(),
        label: model.label.clone(),
        vendor: model.vendor.clone(),
        keywords: model.keywords.clone(),
        profile: model.profile.clone(),
        icon: model.icon.clone(),
        link: model.link.clone(),
    }
}

/// 显式拆分的型号。未列出的协议自动生成同 id 型号。
const MODELS_JSON: &str = r#"{
  "rainy75-keyboard": {
    "label": "Rainy 75 键盘",
    "vendor": "WOB / Rainy75",
    "icon": "keyboard",
    "link": "2.4G",
    "profile": "rainy75-keyboard",
    "known_ids": [{"vid": "320F", "pid": "5088", "wired_pid": ""}],
    "products": ["RAINY 75", "RAINY75"],
    "keywords": "Rainy 75 WOB"
  },
  "vxe-r1": {
    "label": "VXE R1 鼠标",
    "vendor": "VXE",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "vxe-v3-nk",
    "known_ids": [{"vid": "373B", "pid": "10C9", "wired_pid": "10C7"}],
    "products": ["R1"],
    "keywords": "R1 Nearlink 星闪 蜻蜓"
  },
  "vxe-v3-nk-mouse": {
    "label": "VXE V3 NK 鼠标",
    "vendor": "VXE",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "vxe-v3-nk",
    "known_ids": [{"vid": "373B", "pid": "10C9", "wired_pid": "10C7"}],
    "products": ["V3"],
    "keywords": "V3 NK 蜻蜓 Dragonfly"
  },
  "mchose-g75-pro": {
    "label": "迈丛 G75 Pro 键盘",
    "vendor": "迈丛 MCHOSE",
    "icon": "keyboard",
    "link": "2.4G",
    "profile": "mchose-kb",
    "known_ids": [{"vid": "3554", "pid": "FA09", "wired_pid": ""}, {"vid": "258A", "pid": "01BB", "wired_pid": ""}],
    "products": ["G75 PRO"],
    "keywords": "G75 Pro"
  },
  "mchose-z75s": {
    "label": "迈丛 Z75S 键盘",
    "vendor": "迈丛 MCHOSE",
    "icon": "keyboard",
    "link": "2.4G",
    "profile": "mchose-kb",
    "known_ids": [{"vid": "3554", "pid": "FA09", "wired_pid": ""}, {"vid": "258A", "pid": "01BB", "wired_pid": ""}],
    "products": ["Z75S"],
    "keywords": "Z75S"
  },
  "mchose-k99": {
    "label": "迈丛 K99 键盘",
    "vendor": "迈丛 MCHOSE",
    "icon": "keyboard",
    "link": "2.4G",
    "profile": "mchose-kb",
    "known_ids": [{"vid": "3554", "pid": "FA09", "wired_pid": ""}, {"vid": "258A", "pid": "01BB", "wired_pid": ""}],
    "products": ["K99"],
    "keywords": "K99"
  },
  "mchose-gx87": {
    "label": "迈丛 GX87 键盘",
    "vendor": "迈丛 MCHOSE",
    "icon": "keyboard",
    "link": "2.4G",
    "profile": "mchose-kb",
    "known_ids": [{"vid": "3554", "pid": "FA09", "wired_pid": ""}, {"vid": "258A", "pid": "01BB", "wired_pid": ""}],
    "products": ["GX87"],
    "keywords": "GX87"
  },
  "mchose-g75": {
    "label": "迈丛 G75 键盘",
    "vendor": "迈丛 MCHOSE",
    "icon": "keyboard",
    "link": "2.4G",
    "profile": "mchose-kb",
    "known_ids": [{"vid": "3554", "pid": "FA09", "wired_pid": ""}, {"vid": "258A", "pid": "01BB", "wired_pid": ""}],
    "products": ["G75"],
    "keywords": "G75"
  },
  "leobog-hi75c-pro": {
    "label": "LEOBOG Hi75c Pro 键盘",
    "vendor": "LEOBOG",
    "icon": "keyboard",
    "link": "2.4G",
    "profile": "leobog-hi75c",
    "known_ids": [{"vid": "3554", "pid": "FA09", "wired_pid": ""}],
    "products": ["HI75C"],
    "keywords": "Hi75c Pro"
  },
  "leobog-hi98-pro": {
    "label": "LEOBOG HI98PRO 键盘",
    "vendor": "LEOBOG",
    "icon": "keyboard",
    "link": "2.4G",
    "profile": "leobog-hi75c",
    "known_ids": [{"vid": "3554", "pid": "FA09", "wired_pid": ""}],
    "products": ["HI98"],
    "keywords": "HI98PRO"
  },
  "vxe-r1-pro-max": {
    "label": "VXE R1 Pro Max 鼠标",
    "vendor": "VXE",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "vxe-r1-family",
    "known_ids": [{"vid": "3554", "pid": "F58A", "wired_pid": "F58C"}],
    "products": ["R1 PRO MAX"],
    "keywords": "R1 Pro Max"
  },
  "vxe-r1-se-plus": {
    "label": "VXE R1 SE+ 鼠标",
    "vendor": "VXE",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "vxe-r1-family",
    "known_ids": [{"vid": "3554", "pid": "F58E", "wired_pid": "F58F"}],
    "products": ["R1 SE"],
    "keywords": "R1 SE+"
  },
  "vgn-dragonfly3-pro-max": {
    "label": "VGN 蜻蜓 3 Pro Max 鼠标",
    "vendor": "VGN",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "vgn-ryms",
    "known_ids": [{"vid": "391D", "pid": "1A04", "wired_pid": ""}, {"vid": "391D", "pid": "1A08", "wired_pid": ""}, {"vid": "391D", "pid": "1A13", "wired_pid": ""}, {"vid": "391D", "pid": "1A25", "wired_pid": ""}, {"vid": "391D", "pid": "1A2B", "wired_pid": ""}, {"vid": "391D", "pid": "1A41", "wired_pid": ""}],
    "products": ["3 PRO MAX"],
    "keywords": "蜻蜓 3 Pro Max"
  },
  "vgn-y2-v2-pro-max": {
    "label": "VGN Y2 V2 Pro Max 鼠标",
    "vendor": "VGN",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "vgn-ryms",
    "known_ids": [{"vid": "391D", "pid": "1A04", "wired_pid": ""}, {"vid": "391D", "pid": "1A08", "wired_pid": ""}, {"vid": "391D", "pid": "1A13", "wired_pid": ""}, {"vid": "391D", "pid": "1A25", "wired_pid": ""}, {"vid": "391D", "pid": "1A2B", "wired_pid": ""}, {"vid": "391D", "pid": "1A41", "wired_pid": ""}],
    "products": ["Y2 V2"],
    "keywords": "Y2 V2 Pro Max"
  },
  "vgn-f1-v2-pro-max": {
    "label": "VGN F1 V2 Pro Max 鼠标",
    "vendor": "VGN",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "vgn-ryms",
    "known_ids": [{"vid": "391D", "pid": "1A04", "wired_pid": ""}, {"vid": "391D", "pid": "1A08", "wired_pid": ""}, {"vid": "391D", "pid": "1A13", "wired_pid": ""}, {"vid": "391D", "pid": "1A25", "wired_pid": ""}, {"vid": "391D", "pid": "1A2B", "wired_pid": ""}, {"vid": "391D", "pid": "1A41", "wired_pid": ""}],
    "products": ["F1 V2"],
    "keywords": "F1 V2 Pro Max"
  },
  "vgn-dragonfly3-ultra": {
    "label": "VGN 蜻蜓 3 Ultra 鼠标",
    "vendor": "VGN",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "vgn-ryms",
    "known_ids": [{"vid": "391D", "pid": "1A04", "wired_pid": ""}, {"vid": "391D", "pid": "1A08", "wired_pid": ""}, {"vid": "391D", "pid": "1A13", "wired_pid": ""}, {"vid": "391D", "pid": "1A25", "wired_pid": ""}, {"vid": "391D", "pid": "1A2B", "wired_pid": ""}, {"vid": "391D", "pid": "1A41", "wired_pid": ""}],
    "products": ["3 ULTRA"],
    "keywords": "蜻蜓 3 Ultra"
  },
  "vgn-f1-v5-ultra": {
    "label": "VGN F1 V5 Ultra 鼠标",
    "vendor": "VGN",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "vgn-ryms",
    "known_ids": [{"vid": "391D", "pid": "1A04", "wired_pid": ""}, {"vid": "391D", "pid": "1A08", "wired_pid": ""}, {"vid": "391D", "pid": "1A13", "wired_pid": ""}, {"vid": "391D", "pid": "1A25", "wired_pid": ""}, {"vid": "391D", "pid": "1A2B", "wired_pid": ""}, {"vid": "391D", "pid": "1A41", "wired_pid": ""}],
    "products": ["F1 V5"],
    "keywords": "F1 V5 Ultra"
  },
  "vgn-f2-v2-ultra": {
    "label": "VGN F2 V2 Ultra 鼠标",
    "vendor": "VGN",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "vgn-ryms",
    "known_ids": [{"vid": "391D", "pid": "1A04", "wired_pid": ""}, {"vid": "391D", "pid": "1A08", "wired_pid": ""}, {"vid": "391D", "pid": "1A13", "wired_pid": ""}, {"vid": "391D", "pid": "1A25", "wired_pid": ""}, {"vid": "391D", "pid": "1A2B", "wired_pid": ""}, {"vid": "391D", "pid": "1A41", "wired_pid": ""}],
    "products": ["F2 V2"],
    "keywords": "F2 V2 Ultra"
  },
  "atk-f1-pro-max": {
    "label": "ATK F1 Pro Max 鼠标",
    "vendor": "ATK",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "atk-f1-family",
    "known_ids": [{"vid": "373B", "pid": "101B", "wired_pid": ""}, {"vid": "373B", "pid": "101A", "wired_pid": ""}, {"vid": "373B", "pid": "1015", "wired_pid": ""}, {"vid": "373B", "pid": "1303", "wired_pid": ""}, {"vid": "373B", "pid": "12A9", "wired_pid": ""}, {"vid": "373B", "pid": "11D9", "wired_pid": ""}, {"vid": "373B", "pid": "11E4", "wired_pid": ""}, {"vid": "373B", "pid": "1085", "wired_pid": ""}],
    "products": ["F1 PRO MAX"],
    "keywords": "F1 Pro Max"
  },
  "atk-f1-v3": {
    "label": "ATK F1 V3 鼠标",
    "vendor": "ATK",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "atk-f1-family",
    "known_ids": [{"vid": "373B", "pid": "101B", "wired_pid": ""}, {"vid": "373B", "pid": "101A", "wired_pid": ""}, {"vid": "373B", "pid": "1015", "wired_pid": ""}, {"vid": "373B", "pid": "1303", "wired_pid": ""}, {"vid": "373B", "pid": "12A9", "wired_pid": ""}, {"vid": "373B", "pid": "11D9", "wired_pid": ""}, {"vid": "373B", "pid": "11E4", "wired_pid": ""}, {"vid": "373B", "pid": "1085", "wired_pid": ""}],
    "products": ["F1 V3"],
    "keywords": "F1 V3"
  },
  "atk-f1-ultimate": {
    "label": "ATK F1 Ultimate 鼠标",
    "vendor": "ATK",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "atk-f1-family",
    "known_ids": [{"vid": "373B", "pid": "101B", "wired_pid": ""}, {"vid": "373B", "pid": "101A", "wired_pid": ""}, {"vid": "373B", "pid": "1015", "wired_pid": ""}, {"vid": "373B", "pid": "1303", "wired_pid": ""}, {"vid": "373B", "pid": "12A9", "wired_pid": ""}, {"vid": "373B", "pid": "11D9", "wired_pid": ""}, {"vid": "373B", "pid": "11E4", "wired_pid": ""}, {"vid": "373B", "pid": "1085", "wired_pid": ""}],
    "products": ["F1 ULTIMATE"],
    "keywords": "F1 Ultimate"
  },
  "asus-rog-omni-harpe": {
    "label": "华硕 ROG HARPE 鼠标",
    "vendor": "华硕 ROG",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "asus-rog-omni-mouse",
    "known_ids": [{"vid": "0B05", "pid": "1ACE", "wired_pid": ""}],
    "products": ["HARPE"],
    "keywords": "HARPE Omni"
  },
  "asus-rog-omni-keris": {
    "label": "华硕 ROG KERIS 鼠标",
    "vendor": "华硕 ROG",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "asus-rog-omni-mouse",
    "known_ids": [{"vid": "0B05", "pid": "1ACE", "wired_pid": ""}],
    "products": ["KERIS"],
    "keywords": "KERIS Omni"
  },
  "hyperx-cloud3": {
    "label": "HyperX Cloud III Wireless",
    "vendor": "HyperX",
    "icon": "headset",
    "link": "2.4G",
    "profile": "hyperx-cloud3-wireless",
    "known_ids": [{"vid": "03F0", "pid": "05B7", "wired_pid": ""}, {"vid": "03F0", "pid": "0C9D", "wired_pid": ""}, {"vid": "03F0", "pid": "0FCC", "wired_pid": ""}],
    "products": ["CLOUD III"],
    "keywords": "Cloud III"
  },
  "hyperx-cloud2-core": {
    "label": "HyperX Cloud II Core Wireless",
    "vendor": "HyperX",
    "icon": "headset",
    "link": "2.4G",
    "profile": "hyperx-cloud3-wireless",
    "known_ids": [{"vid": "03F0", "pid": "0995", "wired_pid": ""}, {"vid": "03F0", "pid": "0795", "wired_pid": ""}],
    "products": ["CLOUD II"],
    "keywords": "Cloud II Core"
  },
  "pulsar-x2": {
    "label": "Pulsar X2 鼠标",
    "vendor": "Pulsar 派世",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "pulsar-x2",
    "known_ids": [{"vid": "3554", "pid": "F508", "wired_pid": ""}, {"vid": "3554", "pid": "F507", "wired_pid": ""}, {"vid": "3710", "pid": "5406", "wired_pid": ""}, {"vid": "3710", "pid": "3414", "wired_pid": ""}, {"vid": "3710", "pid": "5501", "wired_pid": ""}],
    "products": ["X2"],
    "keywords": "X2"
  },
  "pulsar-link-8k": {
    "label": "Pulsar Link 8K 鼠标",
    "vendor": "Pulsar 派世",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "pulsar-x2",
    "known_ids": [{"vid": "3554", "pid": "F508", "wired_pid": ""}, {"vid": "3554", "pid": "F507", "wired_pid": ""}, {"vid": "3710", "pid": "5406", "wired_pid": ""}, {"vid": "3710", "pid": "3414", "wired_pid": ""}, {"vid": "3710", "pid": "5501", "wired_pid": ""}],
    "products": ["LINK"],
    "keywords": "Link 8K"
  },
  "mchose-a7-v3-pro": {
    "label": "迈丛 A7 V3 Pro 鼠标",
    "vendor": "迈丛 MCHOSE",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "mchose-a7-v3",
    "known_ids": [{"vid": "3837", "pid": "1018", "wired_pid": ""}, {"vid": "3837", "pid": "1014", "wired_pid": ""}],
    "products": ["A7 V3"],
    "keywords": "A7 V3 Pro"
  },
  "mchose-a7-v3-ultra": {
    "label": "迈丛 A7 V3 Ultra+ 鼠标",
    "vendor": "迈丛 MCHOSE",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "mchose-a7-v3",
    "known_ids": [{"vid": "3837", "pid": "1018", "wired_pid": ""}, {"vid": "3837", "pid": "1014", "wired_pid": ""}],
    "products": ["A7 V3 ULTRA"],
    "keywords": "A7 V3 Ultra+"
  },
  "taidu-tsg809": {
    "label": "钛度 TSG809 鼠标",
    "vendor": "钛度 TAIDU",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "taidu-tsg809",
    "known_ids": [{"vid": "A8A5", "pid": "", "wired_pid": ""}, {"vid": "A8A4", "pid": "", "wired_pid": ""}, {"vid": "3A37", "pid": "", "wired_pid": ""}],
    "products": ["TSG809"],
    "keywords": "TSG809"
  },
  "hjsdzhi-s1-v2": {
    "label": "HJSDZHI S1-V2 鼠标",
    "vendor": "钛度 TAIDU",
    "icon": "mouse",
    "link": "2.4G",
    "profile": "taidu-tsg809",
    "known_ids": [{"vid": "A8A5", "pid": "", "wired_pid": ""}, {"vid": "A8A4", "pid": "", "wired_pid": ""}, {"vid": "3A37", "pid": "", "wired_pid": ""}],
    "products": ["S1-V2"],
    "keywords": "S1-V2"
  }
}"#;

fn guess_icon(label: &str) -> String {
    let low = label.to_lowercase();
    if low.contains("键盘") || low.contains("keyboard") {
        "keyboard".to_string()
    } else if low.contains("耳塞") || low.contains("earbuds") {
        "earbuds".to_string()
    } else if low.contains("耳机") || low.contains("headset") || low.contains("audio") {
        "headset".to_string()
    } else if low.contains("手柄") || low.contains("gamepad") {
        "gamepad".to_string()
    } else {
        "mouse".to_string()
    }
}

fn default_link(label: &str) -> String {
    if label.contains("蓝牙") {
        "蓝牙".to_string()
    } else {
        "2.4G".to_string()
    }
}

fn normalize_id(id: &str) -> String {
    id.trim().to_uppercase()
}

fn normalize_model(mut model: Model) -> Option<Model> {
    model.id = model.id.trim().to_string();
    model.label = model.label.trim().to_string();
    model.profile = model.profile.trim().to_lowercase();
    if model.id.is_empty() || model.label.is_empty() || model.profile.is_empty() {
        return None;
    }
    for known in &mut model.known_ids {
        known.vid = normalize_id(&known.vid);
        known.pid = normalize_id(&known.pid);
        known.wired_pid = normalize_id(&known.wired_pid);
    }
    model.known_ids.retain(|known| !known.vid.is_empty());
    if model.icon.is_empty() {
        model.icon = guess_icon(&model.label);
    }
    if model.link.is_empty() {
        model.link = default_link(&model.label);
    }
    model.products = model
        .products
        .into_iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect();
    Some(model)
}

fn synthetic(profile: &Profile) -> Model {
    Model {
        id: profile.id.clone(),
        label: profile.label.clone(),
        vendor: profile.vendor.clone(),
        icon: guess_icon(&profile.label),
        link: default_link(&profile.label),
        profile: profile.id.clone(),
        known_ids: profile.known_ids.clone(),
        model_code: profile.model_code,
        products: Vec::new(),
        keywords: profile.keywords.clone(),
    }
}

/// 载入显式型号 + 协议表里没有显式条目的自动型号。
pub fn load(profiles: &HashMap<String, Profile>) -> HashMap<String, Model> {
    let mut models = HashMap::new();
    let explicit: HashMap<String, Model> =
        serde_json::from_str(MODELS_JSON).unwrap_or_else(|_| HashMap::new());
    let mut covered: HashSet<String> = HashSet::new();
    for (id, model) in explicit {
        let mut model = model;
        model.id = id;
        if let Some(model) = normalize_model(model) {
            covered.insert(model.profile.clone());
            models.insert(model.id.clone(), model);
        }
    }
    for profile in profiles.values() {
        if covered.contains(&profile.id) {
            continue;
        }
        let model = synthetic(profile);
        models.entry(model.id.clone()).or_insert(model);
    }
    models
}

fn candidate_pids(pid: &str, wired_pid: &str) -> Vec<String> {
    let mut pids = Vec::new();
    for value in [pid, wired_pid] {
        let value = value.trim().to_uppercase();
        if !value.is_empty() && !pids.contains(&value) {
            pids.push(value);
        }
    }
    pids
}

/// 同 VID/PID 的型号候选；精确 PID 优先，没有精确命中时退回同 VID 的通配型号。
pub fn for_ids<'a>(
    vid: &str,
    pid: &str,
    wired_pid: &str,
    table: &'a HashMap<String, Model>,
) -> Vec<&'a Model> {
    let vid = vid.trim().to_uppercase();
    if vid.is_empty() {
        return Vec::new();
    }
    let pids = candidate_pids(pid, wired_pid);
    let mut exact: Vec<&Model> = table
        .values()
        .filter(|model| {
            model
                .known_ids
                .iter()
                .any(|known| known.vid == vid && !known.pid.is_empty() && pids.contains(&known.pid))
        })
        .collect();
    if exact.is_empty() {
        exact = table
            .values()
            .filter(|model| {
                model
                    .known_ids
                    .iter()
                    .any(|known| known.vid == vid && known.pid.is_empty())
            })
            .collect();
    }
    exact.sort_by(|a, b| a.id.cmp(&b.id));
    exact
}

/// 识别优先级：型号码 > 产品名 > 唯一候选；多个候选留给添加窗。
pub fn pick<'a>(
    vid: &str,
    pid: &str,
    wired_pid: &str,
    model_code: Option<u16>,
    product: &str,
    table: &'a HashMap<String, Model>,
) -> ModelPick<'a> {
    let candidates = for_ids(vid, pid, wired_pid, table);
    if candidates.is_empty() {
        return ModelPick {
            candidates,
            selected: None,
        };
    }
    if let Some(code) = model_code {
        let by_code: Vec<&Model> = candidates
            .iter()
            .copied()
            .filter(|model| model.model_code == Some(code))
            .collect();
        if by_code.len() == 1 {
            return ModelPick {
                selected: by_code.first().copied(),
                candidates,
            };
        }
        if !by_code.is_empty() {
            return ModelPick {
                candidates: by_code,
                selected: None,
            };
        }
    }
    let product = product.trim().to_lowercase();
    if !product.is_empty() {
        let scored: Vec<(&Model, usize)> = candidates
            .iter()
            .copied()
            .filter_map(|model| {
                model
                    .products
                    .iter()
                    .filter(|pattern| product.contains(*pattern))
                    .map(String::len)
                    .max()
                    .map(|length| (model, length))
            })
            .collect();
        if let Some(best) = scored.iter().map(|(_, length)| *length).max() {
            let by_product: Vec<&Model> = scored
                .iter()
                .filter(|(_, length)| *length == best)
                .map(|(model, _)| *model)
                .collect();
            if by_product.len() == 1 {
                return ModelPick {
                    selected: by_product.first().copied(),
                    candidates,
                };
            }
            let mut by_product = by_product;
            by_product.sort_by(|a, b| a.id.cmp(&b.id));
            return ModelPick {
                candidates: by_product,
                selected: None,
            };
        }
    }
    if candidates.len() == 1 {
        return ModelPick {
            selected: candidates.first().copied(),
            candidates,
        };
    }
    ModelPick {
        candidates,
        selected: None,
    }
}
