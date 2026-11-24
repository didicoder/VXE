//! 设备协议注册表（严格移植 V16 `src/profiles.py`）。
//!
//! 一个 profile 只描述「怎么读」，不描述「是哪只设备」——设备身份（vid/pid）在
//! config.json 的 devices 里。换同协议型号只改 config，换协议只加 JSON。
//!
//! 内置 profile 用 JSON 常量（随二进制打包，永远不丢）；外置 profile 放
//! `device_profiles/*.json`，同 id 整体覆盖内置，新 id 追加，`enabled:false` 移除。
//!
//! 与 V16 一致的行为细节（不要「顺手修正」）：
//! - `schema_version` 只接受 1；
//! - `response.status.offset` / `response.charging.offset` 默认 `None`（**不是** 2 / 7）；
//! - `scale: 0` 被 `or 1.0` 吞掉按 1.0，而 `scale: "0"` 仍报「不能为 0」；
//! - `require_pid_hit` 命名遗留，含义与字面相反，本模块只忠实保存该布尔值，
//!   反向用法在 `hid` 挑接口时体现；
//! - 同 id 整体覆盖，外置没写的字段回落默认值，不继承内置值。

use super::DeviceSpec;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_PROFILE: &str = "vxe-v3-nk";

/// 内置协议：`vxe-v3-nk` 与 `profiles.py` 的 `BUILTIN` 逐字段一致；
/// 其余为社区开源项目实测/公开协议整理（2.4G-battery-viewer / mouse-battery-monitor / GearPower）。
const BUILTIN_JSON: &str = r#"{
  "atk-a87-pro": {
    "id": "atk-a87-pro",
    "label": "艾泰克 ATK A87 Pro 键盘",
    "vendor": "艾泰克 ATK",
    "keywords": "A87 PRO keyboard 键盘 ATK Pro",
    "notes": "协议事实来源 FerrisPulse a916c657c18034a30300dd08b33c6c192c86fb06 src/Hardware/AtkA87ProHelper.cs。373B:111F FF60:0061，33B Output/Input；02 96 24 联合校验；0电量且0状态为休眠帧。仅查询，不含设置命令；本地尚未真实硬件验收。",
    "known_ids": [
      {
        "vid": "373B",
        "pid": "111F",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF60",
      "usage": "0x0061"
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 02 96 24",
      "out_len": 33,
      "in_len": 33,
      "read_matching": true
    },
    "response": {
      "min_length": 33,
      "exact_length": 33,
      "markers": [
        [
          1,
          2
        ],
        [
          2,
          150
        ],
        [
          3,
          36
        ]
      ],
      "level": {
        "offset": 4
      },
      "charging": {
        "offset": 5,
        "mask": 1,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 2,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "hyperx-cloud-iii-s-02cc": {
    "id": "hyperx-cloud-iii-s-02cc",
    "label": "极度未知 HyperX Cloud III S 耳机",
    "vendor": "极度未知 HyperX",
    "keywords": "Cloud III S 02CC 耳机 HyperX Receiver",
    "notes": "协议事实来源 HyperBatteryMonitor 999d40bc1502d6e932c5c3df4e6a32d5f00c7158 Devices/Cloud3SAlternateReceiverDevice.cs。精确03F0:02CC、01C0:0001；仅查询selector06电量。充电selector48未接入；未推断其他接收器变体。本地尚未真实硬件验收。",
    "known_ids": [
      {
        "vid": "03F0",
        "pid": "02CC",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0x01C0",
      "usage": "0x0001"
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "0C 02 03 01 00 06",
      "read_matching": true
    },
    "response": {
      "min_length": 7,
      "markers": [
        [
          0,
          12
        ],
        [
          1,
          2
        ],
        [
          2,
          3
        ],
        [
          5,
          6
        ]
      ],
      "level": {
        "offset": 6
      },
      "charging": {
        "offset": null
      }
    },
    "sampling": {
      "attempts": 2,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "rainy75-keyboard": {
    "schema_version": 1,
    "label": "WOB Rainy 75 键盘",
    "vendor": "WOB",
    "keywords": "Rainy 75 WOB 键盘",
    "notes": "来源 Rainy75Desktop：320F:5088、FF1C:0092、Report ID 04；查询帧 04 20 00 1A 06，回包 [3]=1A、[8]=电量、[9]=充电状态。与 VGN V98 Pro 共用 VID/PID，添加时需由用户按型号选择。该协议源于网页驱动逆向。",
    "known_ids": [
      {
        "vid": "320F",
        "pid": "5088",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF1C",
      "usage": "0x0092",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "04 20 00 1A 06",
      "out_len": null,
      "in_len": null,
      "report_id": null,
      "read_matching": true
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 3,
        "ok_values": [
          26
        ]
      },
      "level": {
        "offset": 8,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 9,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "vxe-v3-nk": {
    "schema_version": 1,
    "label": "威界 VXE V3 NK / R1 Nearlink 鼠标",
    "keywords": "VXE V3 蜻蜓 V3 Dragonfly R1 Nearlink 星闪 NANO 鼠标 NK",
    "vendor": "威界 VXE",
    "notes": "接收器 373B:10C9；17 字节报文应答，resp[6]=电量，resp[7]!=0 为有线/充电。",
    "known_ids": [
      {
        "vid": "373B",
        "pid": "10C9",
        "wired_pid": "10C7"
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 7,
      "status": {
        "offset": 2,
        "ok_values": [
          0
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "atk-vxe-nordic54": {
    "schema_version": 1,
    "label": "艾泰克 ATK / 威界 VXE Nordic 54 鼠标",
    "vendor": "艾泰克 ATK / 威界 VXE",
    "notes": "来源 2.4G-battery-viewer 实测：usage 0xFF05，Report 0x08；回包 resp[1]=0x72、resp[5]=0x07，电量 resp[7]。设备休眠时无应答，动一下鼠标再试。",
    "known_ids": [
      {
        "vid": "373B",
        "pid": "1216",
        "wired_pid": ""
      },
      {
        "vid": "373B",
        "pid": "1155",
        "wired_pid": "1154"
      },
      {
        "vid": "373B",
        "pid": "124F",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF05",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 7D 72 02 00 01 07 01",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 8,
      "status": {
        "offset": 1,
        "ok_values": [
          114
        ]
      },
      "level": {
        "offset": 7,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "ATK VXE 无线鼠标 Nordic 54"
  },
  "vaxee-3057": {
    "schema_version": 1,
    "label": "VAXEE / ZYGEN 鼠标",
    "vendor": "VAXEE",
    "notes": "来源 mouse-battery-monitor VAXEE.md 实测协议：Report 0x0E，命令 0x0B 读电量，回包 offset5 = 0..20 档（×5 = 百分比）。充电需另一命令 0x10，本协议未含。",
    "known_ids": [
      {
        "vid": "3057",
        "pid": "1001",
        "wired_pid": ""
      },
      {
        "vid": "3057",
        "pid": "1002",
        "wired_pid": ""
      },
      {
        "vid": "3057",
        "pid": "0005",
        "wired_pid": ""
      },
      {
        "vid": "3057",
        "pid": "2001",
        "wired_pid": ""
      },
      {
        "vid": "3057",
        "pid": "1003",
        "wired_pid": ""
      },
      {
        "vid": "3057",
        "pid": "1004",
        "wired_pid": ""
      },
      {
        "vid": "3057",
        "pid": "1005",
        "wired_pid": ""
      },
      {
        "vid": "3057",
        "pid": "1006",
        "wired_pid": ""
      },
      {
        "vid": "3057",
        "pid": "1007",
        "wired_pid": ""
      },
      {
        "vid": "3057",
        "pid": "1008",
        "wired_pid": ""
      },
      {
        "vid": "3057",
        "pid": "1009",
        "wired_pid": ""
      },
      {
        "vid": "3057",
        "pid": "1010",
        "wired_pid": ""
      },
      {
        "vid": "3057",
        "pid": "1011",
        "wired_pid": ""
      },
      {
        "vid": "3057",
        "pid": "1012",
        "wired_pid": ""
      },
      {
        "vid": "3057",
        "pid": "1013",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF05",
      "usage": "0x01",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "0E A5 0B 01 01",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 6,
      "status": {
        "offset": 2,
        "ok_values": [
          11
        ]
      },
      "level": {
        "offset": 5,
        "encoding": "u8",
        "min": 0,
        "max": 20,
        "scale": 5,
        "add": 0
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "VAXEE ZYGEN 无线鼠标"
  },
  "asus-rog-pelta": {
    "schema_version": 1,
    "label": "华硕 ROG PELTA / DELTA II 耳机",
    "keywords": "ROG DELTA II PELTA 耳机 华硕",
    "vendor": "华硕 ROG",
    "notes": "来源 GearPower（g-helper 协议）：OUT CC 12 07，回包同前缀，电量 offset 6。Voltide 交叉确认（DELTA II 1AFA/1D41、PELTA 1B82/1B84，SpeedNova 一代共用帧）；充电需另发 CC 12 08（单查询 schema 暂不含）。",
    "known_ids": [
      {
        "vid": "0B05",
        "pid": "1B84",
        "wired_pid": ""
      },
      {
        "vid": "0B05",
        "pid": "1B82",
        "wired_pid": ""
      },
      {
        "vid": "0B05",
        "pid": "1AFA",
        "wired_pid": ""
      },
      {
        "vid": "0B05",
        "pid": "1D41",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": "0x0001",
      "interface": 3,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "write_mode": "set_output_report",
      "payload_hex": "CC 12 07",
      "out_len": 64,
      "in_len": 64,
      "report_id": null
    },
    "response": {
      "min_length": 7,
      "status": {
        "offset": 0,
        "ok_values": [
          204
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "mchose-kb": {
    "schema_version": 1,
    "label": "迈丛 G75 Pro / Z75S / K99 / GX87 / G75 键盘",
    "vendor": "迈丛 MCHOSE",
    "keywords": "迈丛 G75 MCHOSE G75 Z75S K99 GX87 键盘 Pro",
    "notes": "同族接收器共用帧：回包 13 4A 01 00 02…，电量 offset 5、offset 6=16 充电；会话 ACK 型号码分流（K98=0x0212、F99=0x0158），读协议相同。",
    "known_ids": [
      {
        "vid": "3554",
        "pid": "FA09",
        "wired_pid": ""
      },
      {
        "vid": "258A",
        "pid": "01BB",
        "wired_pid": ""
      },
      {
        "vid": "3837",
        "pid": "201B",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x02",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "13 4A 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 5E",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 20,
      "status": {
        "offset": 4,
        "ok_values": [
          2
        ]
      },
      "level": {
        "offset": 5,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 6,
        "when_charging": 16
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "lingbao-k98": {
    "schema_version": 1,
    "label": "凌豹 K98 键盘",
    "vendor": "凌豹 Lingbao",
    "keywords": "凌豹 K98 LINGBAO K98 键盘",
    "notes": "同族接收器共用帧：回包 13 4A 01 00 02…，电量 offset 5、offset 6=16 充电；会话 ACK 型号码分流（K98=0x0212、F99=0x0158），读协议相同。",
    "model_code": 530,
    "known_ids": [
      {
        "vid": "3554",
        "pid": "FA09",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x02",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "13 4A 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 5E",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 20,
      "status": {
        "offset": 4,
        "ok_values": [
          2
        ]
      },
      "level": {
        "offset": 5,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 6,
        "when_charging": 16
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "aula-f99": {
    "schema_version": 1,
    "label": "狼蛛 F99 Pro 键盘",
    "vendor": "狼蛛 AULA",
    "keywords": "AULA F99 Pro 狼蛛 F99 键盘",
    "notes": "同族接收器共用帧：回包 13 4A 01 00 02…，电量 offset 5、offset 6=16 充电；会话 ACK 型号码分流（K98=0x0212、F99=0x0158），读协议相同。",
    "model_code": 344,
    "known_ids": [
      {
        "vid": "3554",
        "pid": "FA09",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x02",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "13 4A 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 5E",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 20,
      "status": {
        "offset": 4,
        "ok_values": [
          2
        ]
      },
      "level": {
        "offset": 5,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 6,
        "when_charging": 16
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "leobog-hi75c": {
    "schema_version": 1,
    "label": "莱奥伯格 LEOBOG Hi75c Pro / HI98PRO 键盘",
    "vendor": "莱奥伯格 LEOBOG",
    "keywords": "LEOBOG 凌咖 Hi75c HI98PRO 键盘 Pro",
    "notes": "同族接收器共用帧：回包 13 4A 01 00 02…，电量 offset 5、offset 6=16 充电；会话 ACK 型号码分流（K98=0x0212、F99=0x0158），读协议相同。",
    "known_ids": [
      {
        "vid": "3554",
        "pid": "FA09",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x02",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "13 4A 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 5E",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 20,
      "status": {
        "offset": 4,
        "ok_values": [
          2
        ]
      },
      "level": {
        "offset": 5,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 6,
        "when_charging": 16
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "vgn-beiying-kb": {
    "schema_version": 1,
    "label": "VGN 北影 2.4G 键盘",
    "vendor": "VGN",
    "keywords": "VGN 北影 beiying 键盘 2.4G",
    "notes": "同族接收器共用帧：回包 13 4A 01 00 02…，电量 offset 5、offset 6=16 充电；会话 ACK 型号码分流（K98=0x0212、F99=0x0158），读协议相同。",
    "known_ids": [
      {
        "vid": "258A",
        "pid": "024F",
        "wired_pid": ""
      },
      {
        "vid": "258A",
        "pid": "027F",
        "wired_pid": ""
      },
      {
        "vid": "258A",
        "pid": "027B",
        "wired_pid": ""
      },
      {
        "vid": "391D",
        "pid": "2A01",
        "wired_pid": ""
      },
      {
        "vid": "391D",
        "pid": "2A02",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x02",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "13 4A 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 5E",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 20,
      "status": {
        "offset": 4,
        "ok_values": [
          2
        ]
      },
      "level": {
        "offset": 5,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 6,
        "when_charging": 16
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "taidu-tsg809": {
    "schema_version": 1,
    "label": "钛度 TSG809 / HJSDZHI S1-V2 鼠标",
    "keywords": "TSG809 HJSDZHI S1-V2 YJX-CHIP 钛度 TAIDU 鼠标",
    "vendor": "钛度 TAIDU",
    "notes": "来源 Voltide（TAIDU 官方 WebHID 驱动逆向）：65 字节 00 55 30 A5 0B 2E 01 01 01；回包 00 AA 30…，电量 offset 9，offset 10 = 1 充电。官方驱动同时接受 A8A5/A8A4/3A37 三族 VID，FF01/0x10 集合。取代 GearPower 的 F0 ED 01 稿。",
    "known_ids": [
      {
        "vid": "A8A5",
        "pid": "",
        "wired_pid": ""
      },
      {
        "vid": "A8A4",
        "pid": "",
        "wired_pid": ""
      },
      {
        "vid": "3A37",
        "pid": "",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF01",
      "usage": "0x10",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 55 30 A5 0B 2E 01 01 01",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 11,
      "status": {
        "offset": 1,
        "ok_values": [
          170
        ]
      },
      "level": {
        "offset": 9,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 10,
        "when_charging": 1
      }
    }
  },
  "vgn-dragonfly3": {
    "schema_version": 1,
    "label": "VGN 蜻蜓 Dragonfly3 SE 鼠标",
    "vendor": "VGN",
    "keywords": "VGN 蜻蜓 Dragonfly3 SE 鼠标",
    "notes": "与钛度同 YJX 帧族：00 55 30 A5 0B 2E 01 01 01；回包电量 offset 9、offset 10=1 充电。391D:1A01。",
    "known_ids": [
      {
        "vid": "391D",
        "pid": "1A01",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF01",
      "usage": "0x10",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 55 30 A5 0B 2E 01 01 01",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 11,
      "status": {
        "offset": 1,
        "ok_values": [
          170
        ]
      },
      "level": {
        "offset": 9,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 10,
        "when_charging": 1
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "atk-z87-keyboard": {
    "schema_version": 1,
    "label": "艾泰克 ATK Z87 系列键盘",
    "vendor": "艾泰克 ATK",
    "notes": "来源 atk-battery-tray 实测：usage 0xFF1C，64 字节 04 11 00 1A，电量 resp[8]；休眠时回显变 04 11 00 FF（resp[3]=0xFF、电量清零），status 校验自动排除休眠帧。同协议适用于 373B:1012 等 ATK 键盘。",
    "known_ids": [
      {
        "vid": "373B",
        "pid": "1012",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF1C",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "04 11 00 1A",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 9,
      "status": {
        "offset": 3,
        "ok_values": [
          26
        ]
      },
      "level": {
        "offset": 8,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "ATK Z87 系列键盘"
  },
  "atk-a9-air": {
    "schema_version": 1,
    "label": "艾泰克 ATK A9 Air 大师+ 鼠标",
    "vendor": "艾泰克 ATK",
    "notes": "来源 Voltide 实测（CompX/Nordic 家族，与 VXE V3 NK 同协议）：17 字节 08 04 + 14x00 + 49；resp[2]=0 状态，resp[6]=电量，resp[7]=1 充电。设备休眠不回话，动一下鼠标再试。373B:1278。",
    "known_ids": [
      {
        "vid": "373B",
        "pid": "1278",
        "wired_pid": "1293"
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 2,
        "ok_values": [
          0
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "A9 Air 大师+ CompX ATK 艾泰克 鼠标"
  },
  "imecoo-p1": {
    "schema_version": 1,
    "label": "魅谷 IMECOO P1 鼠标",
    "vendor": "魅谷 IMECOO",
    "notes": "来源 Voltide 实测（CompX 家族同帧）：17 字节 08 04 + 14x00 + 49；resp[2]=0 状态，resp[6]=电量，resp[7]=1 充电。WriteFile 可用。3554:FB34。",
    "known_ids": [
      {
        "vid": "3554",
        "pid": "FB34",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 2,
        "ok_values": [
          0
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "IMECOO P1"
  },
  "asus-rog-omni-kbd": {
    "schema_version": 1,
    "label": "华硕 ROG Omni AZOTH 系列键盘",
    "vendor": "华硕 ROG",
    "notes": "来源 Voltide 实测：Omni 接收器 0B05:1ACE 键盘通道（FF00）64 字节 02 12 01；回包电量 offset 6，offset 9 = 1 充电。设备休眠不回话。",
    "known_ids": [
      {
        "vid": "0B05",
        "pid": "1ACE",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "02 12 01",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 14,
      "status": {
        "offset": 0,
        "ok_values": [
          2
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 9,
        "when_charging": 1
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "华硕 ROG Omni AZOTH"
  },
  "asus-rog-omni-mouse": {
    "schema_version": 1,
    "label": "华硕 ROG Omni HARPE / KERIS 系列鼠标",
    "vendor": "华硕 ROG",
    "notes": "来源 Voltide 实测：Omni 接收器 0B05:1ACE 鼠标通道（FF01）64 字节 03 12 07；回包电量 offset 5，offset 10 > 0 充电。设备休眠不回话。",
    "known_ids": [
      {
        "vid": "0B05",
        "pid": "1ACE",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF01",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "03 12 07",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 11,
      "status": {
        "offset": 0,
        "ok_values": [
          3
        ]
      },
      "level": {
        "offset": 5,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 10,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "华硕 ROG Omni HARPE KERIS"
  },
  "dareu-a950-air": {
    "schema_version": 1,
    "label": "达尔优 A950 AIR V2 鼠标",
    "keywords": "DAREU 达尔优 A950 AIR V2",
    "vendor": "达尔优 DAREU",
    "notes": "来源 Voltide（DAREU Driver 1.5.7 逆向 + 实机）：17 字节 08 04 + 14x00 + 49（校验和 sum<55?85-sum，0x49 即此查询的值，CompX 同族帧）；回包 08 04 00 00 00 02 PP CS…，status offset5=2，电量 offset 6，offset 7 = 1 充电 / 2 满。",
    "known_ids": [
      {
        "vid": "260D",
        "pid": "1175",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x02",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 5,
        "ok_values": [
          2
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 7,
        "when_charging": 1
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "x7-crociris": {
    "schema_version": 1,
    "label": "CROCIRIS X7 8K 鼠标",
    "vendor": "CROCIRIS",
    "notes": "来源 Voltide（Crociris Hub KM WebHID 逆向）：65 字节 00 AA 12 00 00（命令 0x12 查电量）；回包 00 55 12 00 00 CC LL…，status offset1=0x55，offset 5 = 充电（非 0），电量 offset 6。鼠标休眠不回帧。",
    "known_ids": [
      {
        "vid": "36F9",
        "pid": "AA20",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF0A",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 AA 12 00 00",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 7,
      "status": {
        "offset": 1,
        "ok_values": [
          85
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 5,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "X7 Receiver 8K CROCIRIS 鼠标"
  },
  "xiberia-t7": {
    "schema_version": 1,
    "label": "西伯利亚 T7 / T7 GT 耳机",
    "keywords": "XIBERIA T7 耳机 西伯利亚 GT",
    "vendor": "西伯利亚 XIBERIA",
    "notes": "来源 Voltide（XIBERIA HUB ActHID.dll 逆向）：64 字节 55 65 00 00（报告 ID 0x55，命令 0x65 查电量）；回包 55 65 00 LL…，电量 offset 3；充电未提供。德盛方案，VID 36F9 全系按 FF00 命令口匹配（与 X7 的 FF0A 口不冲突）。",
    "known_ids": [
      {
        "vid": "36F9",
        "pid": "",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "55 65 00 00",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 4,
      "status": {
        "offset": 0,
        "ok_values": [
          85
        ]
      },
      "level": {
        "offset": 3,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "atk-n9v2-ultimate": {
    "schema_version": 1,
    "label": "艾泰克 ATK N9V2 ULTIMATE 耳机",
    "keywords": "N9 V2 ULTRA ULTIMATE 海王星 耳机 ATK N9V2",
    "vendor": "艾泰克 ATK",
    "notes": "来源 Voltide（ATK HUB app.asar bolutek 协议逆向）：FF90 口 64 字节 55 65 01（命令 0x65，目标 0x01 耳机）；回包 55 65 PP SS…，电量 offset 2，offset 3 = 2 放电 / 3 充电 / 4 满。必须走 FF90（FF82 拒收），写入用 WriteFile 非 SetOutputReport。",
    "known_ids": [
      {
        "vid": "373B",
        "pid": "1262",
        "wired_pid": ""
      },
      {
        "vid": "373B",
        "pid": "12D6",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF90",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "55 65 01",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 4,
      "status": {
        "offset": 1,
        "ok_values": [
          101
        ]
      },
      "level": {
        "offset": 2,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 3,
        "when_charging": 3
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "mchose-v9-headset": {
    "schema_version": 1,
    "label": "迈丛 V9 Turbo+ 耳机",
    "keywords": "V9 PRO X9 迈丛 MCHOSE Turbo",
    "vendor": "迈丛 MCHOSE",
    "notes": "来源 Voltide（myfinder-device-battery#61 实机抓包）：FF90 口 64 字节 55 65 01（与 ATK N9V2 同帧）；回包 55 65 PP SS…，电量 offset 2，offset 3 = 2 放电 / 3 充电 / 4 满。伴生 3837:6008 灯效口不用管。",
    "known_ids": [
      {
        "vid": "3837",
        "pid": "600A",
        "wired_pid": ""
      },
      {
        "vid": "291D",
        "pid": "385D",
        "wired_pid": ""
      },
      {
        "vid": "3837",
        "pid": "600D",
        "wired_pid": ""
      },
      {
        "vid": "3837",
        "pid": "6045",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF90",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "55 65 01",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 4,
      "status": {
        "offset": 1,
        "ok_values": [
          101
        ]
      },
      "level": {
        "offset": 2,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 3,
        "when_charging": 3
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "asus-rog-mouse": {
    "schema_version": 1,
    "label": "华硕 ROG Keris AimPoint 等直连鼠标",
    "vendor": "华硕 ROG",
    "notes": "来源 control-center（libratbag asus.c 补 0x0712 电量命令，Keris Wireless AimPoint）：FF01 私有协议口 65 字节 00 12 07（report id 0 + GET_BATTERY 0x0712 小端）；回包 00 12 07 00 00 PP …，电量 raw[5]。充电字节恒为 1 不可信（硬件限制），故不出充电态。注意：该口在部分固件上拒绝 overlapped 句柄（control-center 实测需同步 IO），若读不到可试 2.4G/有线另一路。",
    "known_ids": [
      {
        "vid": "0B05",
        "pid": "",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF01",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 12 07",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 7,
      "status": {
        "offset": 1,
        "ok_values": [
          18
        ]
      },
      "level": {
        "offset": 5,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "华硕 ROG Keris AimPoint 等直连"
  },
  "hyperx-cloud2-wireless": {
    "schema_version": 1,
    "label": "极度未知 HyperX Cloud II Wireless 耳机",
    "vendor": "极度未知 HyperX",
    "notes": "来源 myfinder-device-battery（HeadsetControl hyperx_cloud_2_wireless 同源）：FF90/0303 口 52 字节 06 FF BB 02（cmd 02=电量）；回包 06 FF BB 02…，电量 offset 7。充电需另发 cmd 03（单查询 schema 不含）。0x0696 旧款 / 0x018B 新款。",
    "known_ids": [
      {
        "vid": "03F0",
        "pid": "0696",
        "wired_pid": ""
      },
      {
        "vid": "03F0",
        "pid": "018B",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF90",
      "usage": "0x0303",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "06 FF BB 02",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 8,
      "status": {
        "offset": 3,
        "ok_values": [
          2
        ]
      },
      "level": {
        "offset": 7,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "HyperX Cloud II Wireless"
  },
  "hyperx-cloud3-wireless": {
    "schema_version": 1,
    "label": "极度未知 HyperX Cloud III / II Core Wireless 耳机",
    "vendor": "极度未知 HyperX",
    "notes": "来源 myfinder-device-battery（HyperHeadset cloud_iii_wireless，NGENUITY USBPcap 对照）：FF13/0001 口 62 字节 66 89（cmd 089=电量）；回包 66 89 或 66 0D，电量 offset 4（[2]/[3] 全零为无读数）。充电需另发 66 8A。05B7/0C9D/0FCC=Cloud III，0995/0795=Cloud II Core。部分接收器只收 Feature 报文（write 报错 1 时读不到）。",
    "known_ids": [
      {
        "vid": "03F0",
        "pid": "05B7",
        "wired_pid": ""
      },
      {
        "vid": "03F0",
        "pid": "0C9D",
        "wired_pid": ""
      },
      {
        "vid": "03F0",
        "pid": "0FCC",
        "wired_pid": ""
      },
      {
        "vid": "03F0",
        "pid": "0995",
        "wired_pid": ""
      },
      {
        "vid": "03F0",
        "pid": "0795",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF13",
      "usage": "0x0001",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "66 89",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 5,
      "status": {
        "offset": 1,
        "ok_values": [
          137,
          13
        ]
      },
      "level": {
        "offset": 4,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "HyperX Cloud III Core Wireless"
  },
  "hyperx-alpha2": {
    "schema_version": 1,
    "label": "极度未知 HyperX Cloud Alpha 2 耳机",
    "vendor": "极度未知 HyperX",
    "notes": "来源 myfinder-device-battery（NGENUITY USBPcap #26 双电量对照）：FF13/FF00 口 64 字节 50 02；回包 51 02…，电量 offset 2。充电=offset 6 bit7（位域，schema 无掩码故暂不出充电态）。勿用 61 02 帧（非电量）。03F0:08BE 控制半体（0ABE 是纯音频）。",
    "known_ids": [
      {
        "vid": "03F0",
        "pid": "08BE",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF13",
      "usage": "0xFF00",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "50 02",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 3,
      "status": {
        "offset": 0,
        "ok_values": [
          81
        ]
      },
      "level": {
        "offset": 2,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "HyperX Cloud Alpha 2"
  },
  "compx-ff02-17b": {
    "schema_version": 1,
    "label": "艾泰克 ATK / 威界 VXE 自动识别鼠标",
    "vendor": "艾泰克 ATK / 威界 VXE",
    "keywords": "ATK VXE 其他 未列型号 其他鼠标 自动识别",
    "notes": "CompX 17 字节 08 04…49（校验 0x55-sum）；回包电量 offset 6、offset 7 非零=充电、8..9 毫伏。373B 表外型号自动命中（精确 PID 条目优先）。",
    "known_ids": [
      {
        "vid": "373B",
        "pid": "",
        "wired_pid": ""
      },
      {
        "vid": "373B",
        "pid": "104D",
        "wired_pid": "103F"
      },
      {
        "vid": "373B",
        "pid": "1040",
        "wired_pid": "104C"
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 1,
        "ok_values": [
          4
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "vxe-r1-family": {
    "schema_version": 1,
    "label": "威界 VXE R1 Pro Max / R1 SE 鼠标",
    "vendor": "威界 VXE",
    "keywords": "VXE R1 Pro Max R1 SE R1 SE+ 鼠标",
    "notes": "CompX 17 字节 08 04…49（校验 0x55-sum）；回包电量 offset 6、offset 7 非零=充电、8..9 毫伏。373B 表外型号自动命中（精确 PID 条目优先）。",
    "known_ids": [
      {
        "vid": "3554",
        "pid": "F58A",
        "wired_pid": ""
      },
      {
        "vid": "3554",
        "pid": "F58C",
        "wired_pid": ""
      },
      {
        "vid": "3554",
        "pid": "F58E",
        "wired_pid": ""
      },
      {
        "vid": "3554",
        "pid": "F58F",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 1,
        "ok_values": [
          4
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "atk-f1-family": {
    "schema_version": 1,
    "label": "艾泰克 ATK F1 Pro Max / F1 V3 / F1 Ultimate 鼠标",
    "vendor": "艾泰克 ATK",
    "keywords": "ATK F1 Pro Max F1 V3 F1 Ultimate 海王星 鼠标",
    "notes": "CompX 17 字节 08 04…49（校验 0x55-sum）；回包电量 offset 6、offset 7 非零=充电、8..9 毫伏。373B 表外型号自动命中（精确 PID 条目优先）。",
    "known_ids": [
      {
        "vid": "373B",
        "pid": "101B",
        "wired_pid": ""
      },
      {
        "vid": "373B",
        "pid": "101A",
        "wired_pid": ""
      },
      {
        "vid": "373B",
        "pid": "1015",
        "wired_pid": ""
      },
      {
        "vid": "373B",
        "pid": "1303",
        "wired_pid": ""
      },
      {
        "vid": "373B",
        "pid": "12A9",
        "wired_pid": ""
      },
      {
        "vid": "373B",
        "pid": "11D9",
        "wired_pid": ""
      },
      {
        "vid": "373B",
        "pid": "11E4",
        "wired_pid": ""
      },
      {
        "vid": "373B",
        "pid": "1085",
        "wired_pid": ""
      },
      {
        "vid": "373B",
        "pid": "1031",
        "wired_pid": "102E"
      },
      {
        "vid": "373B",
        "pid": "11B6",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 1,
        "ok_values": [
          4
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "pulsar-x2": {
    "schema_version": 1,
    "label": "派世 Pulsar X2 / Link 8K 鼠标",
    "vendor": "派世 Pulsar",
    "keywords": "Pulsar 派世 X2 V2 Mini CrazyLight Link 8K 鼠标",
    "notes": "CompX 17 字节 08 04…49（校验 0x55-sum）；回包电量 offset 6、offset 7 非零=充电、8..9 毫伏。373B 表外型号自动命中（精确 PID 条目优先）。",
    "known_ids": [
      {
        "vid": "3554",
        "pid": "F508",
        "wired_pid": ""
      },
      {
        "vid": "3554",
        "pid": "F507",
        "wired_pid": ""
      },
      {
        "vid": "3710",
        "pid": "5406",
        "wired_pid": ""
      },
      {
        "vid": "3710",
        "pid": "3414",
        "wired_pid": ""
      },
      {
        "vid": "3710",
        "pid": "5501",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 1,
        "ok_values": [
          4
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "scyrox-8k": {
    "schema_version": 1,
    "label": "赛洛斯 Scyrox 8K 鼠标",
    "vendor": "赛洛斯 Scyrox",
    "keywords": "Scyrox 8K 接收器 鼠标",
    "notes": "CompX 17 字节 08 04…49（校验 0x55-sum）；回包电量 offset 6、offset 7 非零=充电、8..9 毫伏。373B 表外型号自动命中（精确 PID 条目优先）。",
    "known_ids": [
      {
        "vid": "3554",
        "pid": "F517",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 1,
        "ok_values": [
          4
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "hitscan-hyperlight": {
    "schema_version": 1,
    "label": "Hitscan Hyperlight 鼠标",
    "vendor": "Hitscan",
    "keywords": "Hitscan Hyperlight 鼠标",
    "notes": "CompX 17 字节 08 04…49（校验 0x55-sum）；回包电量 offset 6、offset 7 非零=充电、8..9 毫伏。373B 表外型号自动命中（精确 PID 条目优先）。",
    "known_ids": [
      {
        "vid": "3770",
        "pid": "0200",
        "wired_pid": ""
      },
      {
        "vid": "3770",
        "pid": "0100",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 1,
        "ok_values": [
          4
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "vgn-f1-pro": {
    "schema_version": 1,
    "label": "VGN 蜻蜓 F1 Pro 4K 鼠标",
    "vendor": "VGN",
    "keywords": "VGN 蜻蜓 F1 Pro Dragonfly 鼠标 4K",
    "notes": "CompX 17 字节 08 04…49（校验 0x55-sum）；回包电量 offset 6、offset 7 非零=充电、8..9 毫伏。373B 表外型号自动命中（精确 PID 条目优先）。",
    "known_ids": [
      {
        "vid": "3554",
        "pid": "F505",
        "wired_pid": ""
      },
      {
        "vid": "3554",
        "pid": "F502",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 1,
        "ok_values": [
          4
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "vgn-f2-compx": {
    "schema_version": 1,
    "label": "VGN 蜻蜓 F2 大师+ 鼠标",
    "vendor": "VGN",
    "keywords": "VGN 蜻蜓 F2 大师 Dragonfly 鼠标",
    "notes": "CompX 17 字节 08 04…49（校验 0x55-sum）；回包电量 offset 6、offset 7 非零=充电、8..9 毫伏。373B 表外型号自动命中（精确 PID 条目优先）。",
    "known_ids": [
      {
        "vid": "3554",
        "pid": "FB56",
        "wired_pid": ""
      },
      {
        "vid": "3554",
        "pid": "FB57",
        "wired_pid": ""
      },
      {
        "vid": "3554",
        "pid": "FB3D",
        "wired_pid": ""
      },
      {
        "vid": "3554",
        "pid": "FB3E",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 1,
        "ok_values": [
          4
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "vgn-v98-kb": {
    "schema_version": 1,
    "label": "VGN V98 Pro 键盘",
    "vendor": "VGN",
    "keywords": "VGN V98 Pro 键盘",
    "notes": "FF1C/0092 口 64 字节 04 00 00 1A 06；回包 [3]=0x1A、[8]=电量、[9]=0/1 充电。",
    "known_ids": [
      {
        "vid": "320F",
        "pid": "5088",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF1C",
      "usage": "0x0092",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "04 00 00 1A 06",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 3,
        "ok_values": [
          26
        ]
      },
      "level": {
        "offset": 8,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 9,
        "when_charging": 1
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "hecate-g3m": {
    "schema_version": 1,
    "label": "漫步者 HECATE G3M Pro 鼠标",
    "vendor": "漫步者 HECATE",
    "keywords": "HECATE 漫步者 G3M Pro 鼠标",
    "notes": "FF1C/0092 口 64 字节 04 00 00 1A 06；回包 [3]=0x1A、[8]=电量、[9]=0/1 充电。",
    "known_ids": [
      {
        "vid": "320F",
        "pid": "706E",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF1C",
      "usage": "0x0092",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "04 00 00 1A 06",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 3,
        "ok_values": [
          26
        ]
      },
      "level": {
        "offset": 8,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 9,
        "when_charging": 1
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "cherry-mx2-pro": {
    "schema_version": 1,
    "label": "樱桃 CHERRY MX 2.0S WL Pro 键盘",
    "vendor": "樱桃 CHERRY",
    "notes": "来源 myfinder-device-battery（#482 用户对照，差 3-4 个百分点）：FF1C/0092 口 64 字节 04 20 00 1A 06 + 59 零；回包 byte8=电量 1-100、byte9=0/1 充电。046A:01E0=Dongle、046A:01DE=键盘本体（MI_01）。BLE 0687:01DE 走系统蓝牙电量。",
    "known_ids": [
      {
        "vid": "046A",
        "pid": "01E0",
        "wired_pid": ""
      },
      {
        "vid": "046A",
        "pid": "01DE",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF1C",
      "usage": "0x0092",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "04 20 00 1A 06",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 3,
        "ok_values": [
          26
        ]
      },
      "level": {
        "offset": 8,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 9,
        "when_charging": 1
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "CHERRY MX 2.0S WL Pro"
  },
  "lingbao-m1pro": {
    "schema_version": 1,
    "label": "凌豹 M1 Pro 鼠标",
    "vendor": "凌豹 Lingbao",
    "notes": "来源 myfinder-device-battery（#171 两条 65% 帧）：FF00/0001 口 Feature 报告 0x0F（33 字节），单次 GetFeature；byte18=电量 1-100、byte19=0。088D:052F（产品名 Wireless-Receiver，MI_01）。",
    "known_ids": [
      {
        "vid": "088D",
        "pid": "052F",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": "0x0001",
      "require_pid_hit": false
    },
    "request": {
      "transport": "feature_read",
      "payload_hex": "0F",
      "out_len": null,
      "in_len": 33,
      "report_id": 15
    },
    "response": {
      "min_length": 19,
      "status": {
        "offset": 19,
        "ok_values": [
          0
        ]
      },
      "level": {
        "offset": 18,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "凌豹 M1 Pro"
  },
  "moondrop-gm01": {
    "schema_version": 1,
    "label": "水月雨 MOONDROP GM01 Pro 耳机",
    "vendor": "水月雨 MOONDROP",
    "notes": "来源 myfinder-device-battery（#127 用户实机 50%）：FF02/0002 口报告 0x4B，发 4B 80 27 00 + 补零；回包索引 8 非零、索引 9=电量 0-100。35D8:0133。",
    "known_ids": [
      {
        "vid": "35D8",
        "pid": "0133",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "4B 80 27 00",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 0,
        "ok_values": [
          75
        ]
      },
      "level": {
        "offset": 9,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "水月雨 GM01 Pro"
  },
  "mchose-a7-v3": {
    "schema_version": 1,
    "label": "迈丛 A7 V3 Pro / Ultra+ 鼠标",
    "keywords": "K7 V2 Ultra+ A7 V3 迈丛 Pro",
    "vendor": "迈丛 MCHOSE",
    "notes": "来源 myfinder-device-battery（OpenMouse V3 codec，#535 实机 84%）：FF01/0001 口 64 字节 4D 01 01 00 00 09 00 00 08（cmd 0x0900 只读电源查询，带 XOR）；回包 4D 01 …，cmd 回显 [4:6]=00 09，data@8：VID 3837、link 非零、offset 19=充电非零、offset 20=电量 0-100。3837:1018/1014。",
    "known_ids": [
      {
        "vid": "3837",
        "pid": "1018",
        "wired_pid": ""
      },
      {
        "vid": "3837",
        "pid": "1014",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF01",
      "usage": "0x0001",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "4D 01 01 00 00 09 00 00 08",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 21,
      "status": {
        "offset": 5,
        "ok_values": [
          9
        ]
      },
      "level": {
        "offset": 20,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 19,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "hyperx-cloud-alpha-wireless": {
    "schema_version": 1,
    "label": "极度未知 HyperX Cloud Alpha Wireless 耳机",
    "vendor": "极度未知 HyperX",
    "notes": "03F0:098D，FF43/0202口，21 BB 0B查询。3.2.0依据HeadsetControl d8865353 src/devices/hyperx_calphaw.c与HyperBatteryMonitor 999d40bc Devices/CloudAlphaWirelessDevice.cs交叉纠正：电量为回包offset3，不是0x0B命令值对应的offset11。充电必须独立21 BB 0C查询，本单步profile不推断充电；尚无本地硬件验收。",
    "known_ids": [
      {
        "vid": "03F0",
        "pid": "098D",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF43",
      "usage": "0x0202",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "21 BB 0B",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 4,
      "status": {
        "offset": 2,
        "ok_values": [
          11
        ]
      },
      "level": {
        "offset": 3,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": null
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "HyperX Cloud Alpha Wireless"
  },
  "hecate-g5m": {
    "schema_version": 1,
    "label": "漫步者 HECATE G5M Pro 鼠标",
    "keywords": "漫步者 HECATE G5M Pro",
    "vendor": "漫步者 HECATE",
    "notes": "来源 vigil-watt（HECATE Connect 二进制命令表 + 真机 5/5）：FF1C/0092 口 64 字节 04 00 00 1A + 零（0x1A=状态/电量命令）；回包 04 00 00 1A 00 00 00 00 LL CC…，[3]=0x1A 帧标志、[8]=电量、[9]=0 放电/1 充电/2 满。35BB:D00B。",
    "known_ids": [
      {
        "vid": "35BB",
        "pid": "D00B",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF1C",
      "usage": "0x0092",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "04 00 00 1A",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 3,
        "ok_values": [
          26
        ]
      },
      "level": {
        "offset": 8,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 9,
        "when_charging": 1
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "ganss-gs3104": {
    "schema_version": 1,
    "label": "高斯 GS3104T-Pro 键盘",
    "vendor": "高斯 GANSS",
    "keywords": "高斯 GANSS GS3104T Pro 键盘 GS3104T-Pro",
    "notes": "05AC:024F 同指纹（高斯/狼蛛同硅方案），33 字节 00 20 01…21 查询；回包电量 offset 4。品牌按产品名区分，协议相同。",
    "known_ids": [
      {
        "vid": "05AC",
        "pid": "024F",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF60",
      "usage": "0x0061",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 20 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 21",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 5,
      "status": {
        "offset": 1,
        "ok_values": [
          32
        ]
      },
      "level": {
        "offset": 4,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "aula-f108": {
    "schema_version": 1,
    "label": "狼蛛 F108 Pro 键盘",
    "vendor": "狼蛛 AULA",
    "keywords": "狼蛛 AULA F108 Pro 键盘",
    "notes": "05AC:024F 同指纹（高斯/狼蛛同硅方案），33 字节 00 20 01…21 查询；回包电量 offset 4。品牌按产品名区分，协议相同。",
    "known_ids": [
      {
        "vid": "05AC",
        "pid": "024F",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF60",
      "usage": "0x0061",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 20 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 21",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 5,
      "status": {
        "offset": 1,
        "ok_values": [
          32
        ]
      },
      "level": {
        "offset": 4,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "darkrim-shadow": {
    "schema_version": 1,
    "label": "寸渊影弑 DARKRIM SHADOW 耳机",
    "keywords": "影弑 DARKRIM 耳机 寸渊影弑 SHADOW",
    "vendor": "寸渊影弑 DARKRIM",
    "notes": "来源 vigil-watt（真机逐字节 + CRC-16/CCITT 校准）：FF02/0002 口 65 字节 00 FE 00 10 00 02 00 00 03 00 09 01 00 E2 3C（GetBasicInfo TLV09，CRC16 大端；写入须写满 65 字节）；回包 FE 00 11…01 01 LL，子类型=1（电量），载荷字节 bit7=充电、低 7 位=百分比。3654:6901（杰理方案，MI_03 口应答）。",
    "known_ids": [
      {
        "vid": "3654",
        "pid": "6901",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 FE 00 10 00 02 00 00 03 00 09 01 00 E2 3C",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 15,
      "status": {
        "offset": 10,
        "ok_values": [
          1
        ]
      },
      "level": {
        "offset": 12,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0,
        "mask": 127
      },
      "charging": {
        "offset": 12,
        "nonzero": true,
        "mask": 128
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "vgn-f1-kc2": {
    "schema_version": 1,
    "label": "VGN 蜻蜓 F1 V5 Turbo+ 鼠标",
    "vendor": "VGN",
    "notes": "来源 vigil-watt（VGN 网页驱动 kc2 控制器 + 真机）：FFC1/0001 口 64 字节 B3 06 + 零（report 0xB3，0xB5 备用）；回包 B4 06…，配置帧载荷[19]（Windows raw[20]）bit0-6=电量、bit7=充电。391D:1A23（2.4G）/391D:1026（有线，充电时换 PID 两码都认）。",
    "known_ids": [
      {
        "vid": "391D",
        "pid": "1A23",
        "wired_pid": ""
      },
      {
        "vid": "391D",
        "pid": "1026",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFC1",
      "usage": "0x0001",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "B3 06",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 21,
      "status": {
        "offset": 1,
        "ok_values": [
          6
        ]
      },
      "level": {
        "offset": 20,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0,
        "mask": 127
      },
      "charging": {
        "offset": 20,
        "nonzero": true,
        "mask": 128
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "VGN 蜻蜓 F1 V5 Turbo"
  },
  "lingbao-m5pro": {
    "schema_version": 1,
    "label": "凌豹 M5PRO Max 鼠标",
    "keywords": "LINGBAO M5PRO 凌豹 Max",
    "vendor": "凌豹 Lingbao",
    "notes": "来源 vigil-watt（Frida 注入官方驱动抓包修正：命令是 F7 非 0x83）：FFFF/0002 口 feature 65 字节 00 F7 00…（SetFeature→GetFeature，无校验和）；回包 [2]=0 成功、[3]=电量、[5]=1 充电。3151:402D（MI_02；有线 4026 只回显不认）。",
    "known_ids": [
      {
        "vid": "3151",
        "pid": "402D",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFFF",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "feature_write_read",
      "payload_hex": "00 F7",
      "out_len": null,
      "in_len": 65,
      "report_id": 0
    },
    "response": {
      "min_length": 6,
      "status": {
        "offset": 4,
        "ok_values": [
          1
        ]
      },
      "level": {
        "offset": 3,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 5,
        "when_charging": 1
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "eweadn-s9pro": {
    "schema_version": 1,
    "label": "前行者 EWEADN S9 PRO 8K 鼠标",
    "vendor": "前行者 EWEADN",
    "notes": "来源 myfinder-device-battery + vigil-watt（E-HUB 逆向）：FF00/0001 口 feature 64 字节 03 20 50 00 00 4F 81 + 零（SetFeature 后约 100ms GetFeature 03）；回包 03 50 00 0B…LL…A5 5A，字节 6 bit7=充电、低 7 位=电量。372E:1026。",
    "known_ids": [
      {
        "vid": "372E",
        "pid": "1026",
        "wired_pid": ""
      },
      {
        "vid": "372E",
        "pid": "1027",
        "wired_pid": ""
      },
      {
        "vid": "372E",
        "pid": "1029",
        "wired_pid": ""
      },
      {
        "vid": "372E",
        "pid": "103D",
        "wired_pid": ""
      },
      {
        "vid": "372E",
        "pid": "105E",
        "wired_pid": ""
      },
      {
        "vid": "372E",
        "pid": "106E",
        "wired_pid": ""
      },
      {
        "vid": "372E",
        "pid": "107B",
        "wired_pid": ""
      },
      {
        "vid": "372E",
        "pid": "107C",
        "wired_pid": ""
      },
      {
        "vid": "372E",
        "pid": "108A",
        "wired_pid": ""
      },
      {
        "vid": "372E",
        "pid": "108D",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": "0x0001",
      "require_pid_hit": false
    },
    "request": {
      "transport": "feature_write_read",
      "payload_hex": "03 20 50 00 00 4F 81",
      "out_len": null,
      "in_len": 64,
      "report_id": 3
    },
    "response": {
      "min_length": 7,
      "status": {
        "offset": 1,
        "ok_values": [
          80
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0,
        "mask": 127
      },
      "charging": {
        "offset": 6,
        "nonzero": true,
        "mask": 128
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    },
    "keywords": "前行者 EWEADN S9 PRO 8K"
  },
  "vgn-ryms": {
    "schema_version": 1,
    "label": "VGN ryMs 多型号接收器鼠标",
    "vendor": "VGN",
    "keywords": "VGN 蜻蜓 Dragonfly ryMs Y2 F1 F2 3 Pro Max Ultra 接收器 鼠标 2.4G V2 V5",
    "notes": "来源 myfinder-device-battery 1.3.11-beta.12（VGN HUB ryMs 驱动）：391D:1A04/1A08/1A13/1A25/1A2B/1A41，FFFF/0002 口无编号 Feature 65 字节；发 00 F7 + 6x00 + 校验 08，回包 [3]=电量、[5]=1 休眠；充电未知。1A04 贡献者实测 92%/91%。",
    "known_ids": [
      {
        "vid": "391D",
        "pid": "1A04",
        "wired_pid": ""
      },
      {
        "vid": "391D",
        "pid": "1A08",
        "wired_pid": ""
      },
      {
        "vid": "391D",
        "pid": "1A13",
        "wired_pid": ""
      },
      {
        "vid": "391D",
        "pid": "1A25",
        "wired_pid": ""
      },
      {
        "vid": "391D",
        "pid": "1A2B",
        "wired_pid": ""
      },
      {
        "vid": "391D",
        "pid": "1A41",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFFF",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "feature_write_read",
      "payload_hex": "00 F7 00 00 00 00 00 00 08",
      "out_len": null,
      "in_len": 65,
      "report_id": 0
    },
    "response": {
      "min_length": 6,
      "status": {
        "offset": 1,
        "ok_values": [
          0
        ]
      },
      "level": {
        "offset": 3,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "vgn-f1-moba-f503": {
    "schema_version": 1,
    "label": "VGN 蜻蜓 F1 Moba 鼠标",
    "vendor": "VGN",
    "keywords": "VGN 蜻蜓 F1 Moba F503 Dragonfly 鼠标",
    "notes": "来源 myfinder-device-battery（#592 用户完整回复 95%）+ CompX 同族帧：3554:F503 / FF02:0002，17 字节 08 04 + 14x00 + 49；回包电量 offset 6、offset 7 非零=充电。",
    "known_ids": [
      {
        "vid": "3554",
        "pid": "F503",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 1,
        "ok_values": [
          4
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "delux-gh8k": {
    "schema_version": 1,
    "label": "多彩 GH8K / M900 Ultra 2.4G 鼠标",
    "vendor": "多彩 DELUX",
    "keywords": "DELUX 多彩 GH8K M900 Ultra 鼠标 接收器 2.4G",
    "notes": "来源 myfinder-device-battery 1.3.11-beta.12（贡献者 GH8K 90%、用户复验 85%）：3554:FB26 / FF02:0002，双向单一报告 08、17 字节；发 08 04 + 14x00 + 49，回包 status[2]=0、电量[6]；充电位未证实，不显示充电态。",
    "known_ids": [
      {
        "vid": "3554",
        "pid": "FB26",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 17,
      "status": {
        "offset": 2,
        "ok_values": [
          0
        ]
      },
      "level": {
        "offset": 6,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "atk-v75x": {
    "schema_version": 1,
    "label": "艾泰克 ATK V75X 有线键盘",
    "vendor": "艾泰克 ATK",
    "keywords": "ATK V75X Evision 键盘 有线 USB",
    "notes": "来源 HaloBattery 1.17.0（ATK HUB Evision 驱动）：仅收录 320F:5055 有线身份；2.4G 接收器 320F:5088 与 VGN V98 Pro / Rainy75 共用且需产品名分流，当前声明式引擎无法区分，暂不内置。FF1C:0092，32 字节 04 <seq lo> <seq hi> 1A + 28x00；回包 status[3]=1A、电量[8]、state[9] 非零=充电。",
    "known_ids": [
      {
        "vid": "320F",
        "pid": "5055",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF1C",
      "usage": "0x0092",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "04 01 00 1A",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 3,
        "ok_values": [
          26
        ]
      },
      "level": {
        "offset": 8,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 9,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "beitong-kp40d": {
    "schema_version": 1,
    "label": "北通 KP40D 手柄",
    "vendor": "北通 BEITONG",
    "keywords": "北通 BEITONG KP40D 手柄 接收器 2.4G",
    "notes": "来源 HaloBattery 1.17.0（iControl USBPcap）：20BC:515C，00FF/0003 厂商集合；64 字节 02 15 + 62x00；回包 02 15 开头，电量[2]，[4]=0x51 充电 / 0x50 放电（手柄估算值）。",
    "known_ids": [
      {
        "vid": "20BC",
        "pid": "515C",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0x00FF",
      "usage": "0x0003",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "02 15",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 5,
      "status": {
        "offset": 0,
        "ok_values": [
          2
        ]
      },
      "level": {
        "offset": 2,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 4,
        "when_charging": 81
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "asus-rog-speednova": {
    "schema_version": 1,
    "label": "华硕 ROG SPEEDNOVA 8K 鼠标",
    "vendor": "华硕 ROG",
    "keywords": "ROG 华硕 SPEEDNOVA 8K Harpe II Ace 龙鳞 2 Ace 鼠标 等",
    "notes": "来源 g-helper / HaloBattery PR #364 / myfinder-device-battery：0B05:1AD0，FF01/0001 口报告 03、64 字节；发 03 12 07 + 61x00；回包 03 12 07 开头，电量[5]、[10]>0 充电。",
    "known_ids": [
      {
        "vid": "0B05",
        "pid": "1AD0",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF01",
      "usage": "0x0001",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "03 12 07",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 11,
      "status": {
        "offset": 0,
        "ok_values": [
          3
        ]
      },
      "level": {
        "offset": 5,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 10,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  },
  "mchose-g7": {
    "schema_version": 1,
    "label": "迈丛 G7 鼠标",
    "vendor": "迈丛 MCHOSE",
    "keywords": "MCHOSE 迈丛 G7 YJX 鼠标 YJX-CHIP",
    "notes": "来源 myfinder-device-battery（@kek353 实机 46%）：A8A5:2255 / FF01:0010；65 字节 00 55 30 A5 0B 2E 01 01 01 + 零；回包 AA 30 开头，电量[8]、[9] 非零=充电。精确 PID 优先于 TAIDU 的 A8A5 通配，避免把 G7 认成钛度。",
    "known_ids": [
      {
        "vid": "A8A5",
        "pid": "2255",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF01",
      "usage": "0x0010",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 55 30 A5 0B 2E 01 01 01",
      "out_len": null,
      "in_len": null,
      "report_id": null
    },
    "response": {
      "min_length": 10,
      "status": {
        "offset": 0,
        "ok_values": [
          170
        ]
      },
      "level": {
        "offset": 8,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0
      },
      "charging": {
        "offset": 9,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 2,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "mode_nonzero"
    }
  }
}"#;

/// A/B 扩充批次：新增 profile 独立存放，避免改动 57 条内置 JSON 的原有块。
const COMMUNITY_JSON: &str = r#"{
  "mchose-kb-ff04": {
    "schema_version": 1,
    "label": "迈丛 FF04 键盘接收器",
    "vendor": "迈丛 MCHOSE",
    "keywords": "MCHOSE 41E4 键盘接收器 FF04",
    "notes": "myfinder：41E4:2001 / FF04:0002；20 字节 13 4A 01..5E，电量[5]、[6]=16 充电。",
    "known_ids": [
      {
        "vid": "41E4",
        "pid": "2001",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF04",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "13 4A 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 5E",
      "out_len": 20,
      "in_len": 20
    },
    "response": {
      "min_length": 20,
      "status": {
        "offset": 4,
        "ok_values": [
          2
        ]
      },
      "level": {
        "offset": 5
      },
      "charging": {
        "offset": 6,
        "when_charging": 16
      }
    }
  },
  "zaopin-z2-mini": {
    "schema_version": 1,
    "label": "Zaopin Z2 Mini 鼠标",
    "vendor": "Zaopin",
    "keywords": "Zaopin Z2 Mini 鼠标",
    "notes": "Fan4Metal：Compx 17B，3554:F524/F526。",
    "known_ids": [
      {
        "vid": "3554",
        "pid": "F524",
        "wired_pid": "F526"
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 49",
      "out_len": 17,
      "in_len": 17
    },
    "response": {
      "min_length": 7,
      "status": {
        "offset": 2,
        "ok_values": [
          0
        ]
      },
      "level": {
        "offset": 6
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    }
  },
  "steelseries-arctis-7plus": {
    "schema_version": 1,
    "label": "赛睿 SteelSeries Arctis 7+ 耳机",
    "vendor": "赛睿 SteelSeries",
    "keywords": "SteelSeries Arctis 7+ 耳机",
    "notes": "battery-hub：FFC0:0001 / interface 3 / 00 B0；f[1]=1 离线、f[2] 0..4 档、f[3] 充电。",
    "known_ids": [
      {
        "vid": "1038",
        "pid": "220E",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "2212",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "2216",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "2236",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFC0",
      "usage": "0x0001",
      "interface": 3,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 B0",
      "out_len": 64,
      "in_len": 64
    },
    "response": {
      "min_length": 4,
      "status": {
        "offset": 1,
        "ok_values": [
          0
        ]
      },
      "level": {
        "offset": 2,
        "min": 0,
        "max": 4,
        "scale": 25
      },
      "charging": {
        "offset": 3,
        "nonzero": true
      }
    }
  },
  "steelseries-arctis-nova5": {
    "schema_version": 1,
    "label": "赛睿 SteelSeries Arctis Nova 5 耳机",
    "vendor": "赛睿 SteelSeries",
    "keywords": "SteelSeries Arctis Nova 5 3P 耳机",
    "notes": "battery-hub：FFC0:0001 / interface 3 / 00 B0；f[1]=2 离线、f[3] 电量、f[4]=1 充电。",
    "known_ids": [
      {
        "vid": "1038",
        "pid": "2232",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "2253",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "2269",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "226D",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFC0",
      "usage": "0x0001",
      "interface": 3,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 B0",
      "out_len": 64,
      "in_len": 64
    },
    "response": {
      "min_length": 5,
      "status": {
        "offset": 1,
        "ok_values": [
          0
        ]
      },
      "level": {
        "offset": 3
      },
      "charging": {
        "offset": 4,
        "when_charging": 1
      }
    }
  },
  "steelseries-arctis-nova7": {
    "schema_version": 1,
    "label": "赛睿 SteelSeries Arctis Nova 7 耳机",
    "vendor": "赛睿 SteelSeries",
    "keywords": "SteelSeries Arctis Nova 7 7X 7P 耳机",
    "notes": "battery-hub：FFC0:0001 / interface 3 / 00 B0；f[1]=0 在线、f[2] 电量（部分型号 0..4 档）、f[3] 充电。",
    "known_ids": [
      {
        "vid": "1038",
        "pid": "2202",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "2206",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "223A",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "2258",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "227A",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "227E",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "229E",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "22A1",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "22A4",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "22A5",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "22A9",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "22AD",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "220A",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "22A7",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "2298",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFC0",
      "usage": "0x0001",
      "interface": 3,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 B0",
      "out_len": 64,
      "in_len": 64
    },
    "response": {
      "min_length": 4,
      "status": {
        "offset": 1,
        "ok_values": [
          0
        ]
      },
      "level": {
        "offset": 2,
        "min": 0,
        "max": 4,
        "scale": 25
      },
      "charging": {
        "offset": 3,
        "nonzero": true
      }
    }
  },
  "steelseries-arctis1": {
    "schema_version": 1,
    "label": "赛睿 SteelSeries Arctis 1 耳机",
    "vendor": "赛睿 SteelSeries",
    "keywords": "SteelSeries Arctis 1 耳机",
    "notes": "battery-hub：FF43:0202 / interface 3 / 06 12；f[2]=1 离线、f[3] 电量。",
    "known_ids": [
      {
        "vid": "1038",
        "pid": "12B3",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "12B6",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "12D5",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "12D7",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF43",
      "usage": "0x0202",
      "interface": 3,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "06 12",
      "out_len": 64,
      "in_len": 64
    },
    "response": {
      "min_length": 4,
      "status": {
        "offset": 2,
        "ok_values": [
          0
        ]
      },
      "level": {
        "offset": 3
      }
    }
  },
  "steelseries-arctis7": {
    "schema_version": 1,
    "label": "赛睿 SteelSeries Arctis 7 耳机",
    "vendor": "赛睿 SteelSeries",
    "keywords": "SteelSeries Arctis 7 耳机",
    "notes": "battery-hub：FF43:0202 / interface 5 / 06 18；f[2] 电量。",
    "known_ids": [
      {
        "vid": "1038",
        "pid": "1252",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "1260",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "1280",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "12AD",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF43",
      "usage": "0x0202",
      "interface": 5,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "06 18",
      "out_len": 64,
      "in_len": 64
    },
    "response": {
      "min_length": 3,
      "level": {
        "offset": 2
      }
    }
  },
  "steelseries-nova-pro": {
    "schema_version": 1,
    "label": "赛睿 SteelSeries Arctis Nova Pro Wireless 耳机",
    "vendor": "赛睿 SteelSeries",
    "keywords": "SteelSeries Arctis Nova Pro Wireless 耳机",
    "notes": "battery-hub：接口 4 / 06 B0；f[6] 0..8 档、f[15] 充电。",
    "known_ids": [
      {
        "vid": "1038",
        "pid": "12E0",
        "wired_pid": ""
      },
      {
        "vid": "1038",
        "pid": "12E5",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF43",
      "usage": "0x0202",
      "interface": 4,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "06 B0",
      "out_len": 64,
      "in_len": 64
    },
    "response": {
      "min_length": 16,
      "level": {
        "offset": 6,
        "min": 0,
        "max": 8,
        "scale": 12.5
      },
      "charging": {
        "offset": 15,
        "nonzero": true
      }
    }
  },
  "nzxt-lift-elite": {
    "schema_version": 1,
    "label": "恩杰 NZXT Lift Elite 无线鼠标",
    "vendor": "恩杰 NZXT",
    "keywords": "NZXT Lift Elite 鼠标 无线鼠标",
    "notes": "myfinder：FFCA:0001，4E 02 81 00 B0；level[17]、charging[22] bit0、ACK E5。",
    "known_ids": [
      {
        "vid": "1E71",
        "pid": "2101",
        "wired_pid": "2129"
      }
    ],
    "match": {
      "usage_page": "0xFFCA",
      "usage": "0x0001",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "4E 02 81 00 B0",
      "out_len": 64,
      "in_len": 64
    },
    "response": {
      "min_length": 27,
      "status": {
        "offset": 0,
        "ok_values": [
          229
        ]
      },
      "level": {
        "offset": 17
      },
      "charging": {
        "offset": 22,
        "mask": 1,
        "nonzero": true
      }
    }
  },
  "wlmouse-beast": {
    "schema_version": 1,
    "label": "万灵竞记 WLmouse Beast X 鼠标",
    "vendor": "万灵竞记 WLmouse",
    "keywords": "WLmouse Beast X Mini Pro Max 鼠标",
    "notes": "myfinder：FFFF:0000 Feature 65B，00 00 02 02 00 83；a1 00 02 02 00 83 <charging><level>。",
    "known_ids": [
      {
        "vid": "36A7",
        "pid": "A880",
        "wired_pid": ""
      },
      {
        "vid": "36A7",
        "pid": "A887",
        "wired_pid": ""
      },
      {
        "vid": "36A7",
        "pid": "A868",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFFF",
      "usage": "0x0000",
      "require_pid_hit": false
    },
    "request": {
      "transport": "feature_write_read",
      "payload_hex": "00 00 02 02 00 83",
      "out_len": 65,
      "in_len": 65,
      "report_id": 0
    },
    "response": {
      "min_length": 8,
      "markers": [
        [
          2,
          2
        ],
        [
          6,
          131
        ]
      ],
      "status": {
        "offset": 0,
        "ok_values": [
          161
        ]
      },
      "level": {
        "offset": 7
      },
      "charging": {
        "offset": 6,
        "nonzero": true
      }
    }
  },
  "zowie-ec-cw": {
    "schema_version": 1,
    "label": "卓威 Zowie EC-CW 鼠标",
    "vendor": "卓威 Zowie",
    "keywords": "Zowie EC1 EC2 EC3-CW 鼠标 EC-CW",
    "notes": "artnway：04A5:8006/8005，无编号 64B 00 A5 03；[0..1] mV LE、[2] %。",
    "known_ids": [
      {
        "vid": "04A5",
        "pid": "8006",
        "wired_pid": "8005"
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 A5 03",
      "out_len": 64,
      "in_len": 64
    },
    "response": {
      "min_length": 3,
      "level": {
        "offset": 2
      }
    }
  },
  "cooler-master-mm712": {
    "schema_version": 1,
    "label": "酷冷至尊 Cooler Master MM712 鼠标",
    "vendor": "酷冷至尊 Cooler Master",
    "keywords": "Cooler Master MM712 MM731 鼠标",
    "notes": "artnway：interface 3 / FF0A / 00 46 81 00；[10] %、[11] 充电、[12] 链路。",
    "known_ids": [
      {
        "vid": "2516",
        "pid": "0169",
        "wired_pid": ""
      },
      {
        "vid": "2516",
        "pid": "01B7",
        "wired_pid": ""
      },
      {
        "vid": "2516",
        "pid": "0167",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF0A",
      "usage": null,
      "interface": 3,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 46 81 00",
      "out_len": 64,
      "in_len": 64
    },
    "response": {
      "min_length": 13,
      "markers": [
        [
          0,
          71
        ],
        [
          1,
          129
        ]
      ],
      "level": {
        "offset": 10
      },
      "charging": {
        "offset": 11,
        "nonzero": true
      }
    }
  },
  "lamzu-maya-x": {
    "schema_version": 1,
    "label": "兰族 LAMZU Maya X 鼠标",
    "vendor": "兰族 LAMZU",
    "keywords": "LAMZU Maya X Maya X V2 鼠标",
    "notes": "myfinder/artnway：FFFF Feature 65B，00 00 00 02 02 00 83；偏移两处来源差 1，当前取 level[8]/charging[7]。",
    "known_ids": [
      {
        "vid": "373E",
        "pid": "001C",
        "wired_pid": ""
      },
      {
        "vid": "373E",
        "pid": "001D",
        "wired_pid": ""
      },
      {
        "vid": "373E",
        "pid": "001E",
        "wired_pid": ""
      },
      {
        "vid": "37B0",
        "pid": "003A",
        "wired_pid": ""
      },
      {
        "vid": "37B0",
        "pid": "003B",
        "wired_pid": ""
      },
      {
        "vid": "37B0",
        "pid": "0044",
        "wired_pid": ""
      },
      {
        "vid": "373E",
        "pid": "0016",
        "wired_pid": ""
      },
      {
        "vid": "37B0",
        "pid": "001F",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFFF",
      "usage": "0x0000",
      "interface": 2,
      "require_pid_hit": false
    },
    "request": {
      "transport": "feature_write_read",
      "payload_hex": "00 00 00 02 02 00 83",
      "out_len": 65,
      "in_len": 65,
      "report_id": 0
    },
    "response": {
      "min_length": 9,
      "markers": [
        [
          2,
          2
        ],
        [
          6,
          131
        ]
      ],
      "status": {
        "offset": 0,
        "ok_values": [
          161
        ]
      },
      "level": {
        "offset": 8
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    }
  },
  "razer-headset-64b": {
    "schema_version": 1,
    "label": "雷蛇 Razer BlackShark V2 Pro 2023 耳机",
    "vendor": "雷蛇 Razer",
    "keywords": "Razer BlackShark V2 Pro 2023 BlackShark V2 HyperSpeed 耳机",
    "notes": "quiet-battery/LargeAkon：1532:0555/0556/0557/0565，64B 02 80 08…，cmd 21 电量，byte15。",
    "known_ids": [
      {
        "vid": "1532",
        "pid": "0555",
        "wired_pid": ""
      },
      {
        "vid": "1532",
        "pid": "0556",
        "wired_pid": ""
      },
      {
        "vid": "1532",
        "pid": "0557",
        "wired_pid": ""
      },
      {
        "vid": "1532",
        "pid": "0565",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": "0x0001",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "02 80 08 00 00 50 41 08 00 03 21",
      "out_len": 64,
      "in_len": 64
    },
    "response": {
      "min_length": 16,
      "markers": [
        [
          0,
          2
        ],
        [
          1,
          128
        ],
        [
          2,
          8
        ]
      ],
      "level": {
        "offset": 15
      }
    }
  },
  "raptoo-vt-passive": {
    "schema_version": 1,
    "label": "雷柏 VT 系列鼠标",
    "vendor": "雷柏 Rapoo",
    "keywords": "Rapoo 雷柏 VT 鼠标 系列鼠标",
    "notes": "hurttttr/myfinder：24AE / FF00:0002 被动 report 07，[8] 电量、[7] 充电（2 充电）。",
    "known_ids": [
      {
        "vid": "24AE",
        "pid": "",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "passive",
      "in_len": 9
    },
    "passive": {
      "stale_after_s": 15,
      "initial_wait_ms": 2500
    },
    "response": {
      "min_length": 9,
      "status": {
        "offset": 7,
        "ok_values": [
          0,
          1,
          2
        ]
      },
      "level": {
        "offset": 8
      },
      "charging": {
        "offset": 7,
        "when_charging": 2
      }
    }
  },
  "delux-inphic-passive": {
    "schema_version": 1,
    "label": "多彩 / 英菲克 2.4G 鼠标",
    "vendor": "多彩 / 英菲克",
    "keywords": "DELUX Inphic Attack Shark 鼠标 2.4G",
    "notes": "myfinder：1D57:FA60/FA65 被动 5B，03 <model> <event> <power> <percent>。",
    "known_ids": [
      {
        "vid": "1D57",
        "pid": "FA60",
        "wired_pid": ""
      },
      {
        "vid": "1D57",
        "pid": "FA65",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0x000A",
      "usage": "0x0000",
      "require_pid_hit": false
    },
    "request": {
      "transport": "passive",
      "in_len": 5
    },
    "passive": {
      "stale_after_s": 15,
      "initial_wait_ms": 2500
    },
    "response": {
      "min_length": 5,
      "markers": [
        [
          0,
          3
        ]
      ],
      "status": {
        "offset": 3,
        "ok_values": [
          1,
          2,
          3
        ]
      },
      "level": {
        "offset": 4
      }
    }
  },
  "glorious-model-wireless": {
    "schema_version": 1,
    "label": "奥丁（俗称）Glorious Model O/D 无线鼠标",
    "vendor": "奥丁（俗称）Glorious",
    "keywords": "Glorious Model O D Wireless 鼠标 无线鼠标",
    "notes": "artnway：258A Feature 65B，[3]=02 [4]=02 [6]=83；[1] 状态 A1、[6]=83、[7] 充电、[8] 电量。",
    "known_ids": [
      {
        "vid": "258A",
        "pid": "2022",
        "wired_pid": ""
      },
      {
        "vid": "258A",
        "pid": "2011",
        "wired_pid": ""
      },
      {
        "vid": "258A",
        "pid": "2023",
        "wired_pid": ""
      },
      {
        "vid": "258A",
        "pid": "2012",
        "wired_pid": ""
      },
      {
        "vid": "258A",
        "pid": "2024",
        "wired_pid": ""
      },
      {
        "vid": "258A",
        "pid": "2013",
        "wired_pid": ""
      },
      {
        "vid": "258A",
        "pid": "2025",
        "wired_pid": ""
      },
      {
        "vid": "258A",
        "pid": "2014",
        "wired_pid": ""
      },
      {
        "vid": "258A",
        "pid": "2033",
        "wired_pid": ""
      },
      {
        "vid": "258A",
        "pid": "2034",
        "wired_pid": ""
      },
      {
        "vid": "258A",
        "pid": "2036",
        "wired_pid": ""
      },
      {
        "vid": "258A",
        "pid": "201A",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "feature_write_read",
      "payload_hex": "00 00 00 02 02 00 83",
      "out_len": 65,
      "in_len": 65,
      "report_id": 0
    },
    "response": {
      "min_length": 9,
      "markers": [
        [
          6,
          131
        ]
      ],
      "status": {
        "offset": 1,
        "ok_values": [
          161
        ]
      },
      "level": {
        "offset": 8
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    }
  }
}"#;

/// B 类第二批：G-Wolves / Ninjutso / Flydigi / Keychron / Rongyuan / Astro /
/// HyperX Pulsefire / Sony H5 / EWEADN S9 / Kysona / AULA L9 / AJAZZ /
/// Rapoo MT560 / MCHOSE L7 / AULA G9 / HyperX CF2 / Cetra。
const COMMUNITY2_JSON: &str = r#"{
  "g-wolves-new": {
    "schema_version": 1,
    "label": "游狼 G-Wolves 新协议无线鼠标",
    "vendor": "游狼 G-Wolves",
    "keywords": "G-Wolves HTM HSK HTX Fenrir 鼠标 无线鼠标 新协议",
    "notes": "myfinder：Feature 0/65B，00 00 02 02 00 83；A1/A2 状态，level[8]、charging[7]。按 feature_len 65 选集合。",
    "known_ids": [
      {
        "vid": "33E4",
        "pid": "3854",
        "wired_pid": ""
      },
      {
        "vid": "33E4",
        "pid": "3808",
        "wired_pid": "3817"
      },
      {
        "vid": "33E4",
        "pid": "6808",
        "wired_pid": "6817"
      },
      {
        "vid": "33E4",
        "pid": "5608",
        "wired_pid": "5617"
      },
      {
        "vid": "33E4",
        "pid": "3608",
        "wired_pid": "3617"
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": null,
      "feature_len": 65,
      "ignore_usage_page": true,
      "require_pid_hit": true
    },
    "request": {
      "transport": "feature_write_read",
      "payload_hex": "00 00 02 02 00 83 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00",
      "out_len": 65,
      "in_len": 65,
      "report_id": 0
    },
    "response": {
      "min_length": 8,
      "status": {
        "offset": 0,
        "ok_values": [
          161,
          162
        ]
      },
      "level": {
        "offset": 8
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "g-wolves-old": {
    "schema_version": 1,
    "label": "游狼 G-Wolves 旧协议无线鼠标",
    "vendor": "游狼 G-Wolves",
    "keywords": "G-Wolves HTS HTR HSK 鼠标 无线鼠标 旧协议",
    "notes": "myfinder：Feature 0/65B，00 00 02 8F 01；A1 状态，level[6]、charging[5]。",
    "known_ids": [
      {
        "vid": "33E4",
        "pid": "3908",
        "wired_pid": "3917"
      },
      {
        "vid": "33E4",
        "pid": "7904",
        "wired_pid": "7913"
      },
      {
        "vid": "33E4",
        "pid": "3708",
        "wired_pid": "3717"
      },
      {
        "vid": "33E4",
        "pid": "5308",
        "wired_pid": "5317"
      },
      {
        "vid": "33E4",
        "pid": "7704",
        "wired_pid": "7713"
      },
      {
        "vid": "33E4",
        "pid": "3508",
        "wired_pid": "3517"
      },
      {
        "vid": "33E4",
        "pid": "7908",
        "wired_pid": "7917"
      },
      {
        "vid": "33E4",
        "pid": "5808",
        "wired_pid": "5817"
      },
      {
        "vid": "33E4",
        "pid": "2708",
        "wired_pid": "2717"
      },
      {
        "vid": "33E4",
        "pid": "5408",
        "wired_pid": "5417"
      },
      {
        "vid": "33E4",
        "pid": "5708",
        "wired_pid": "5717"
      },
      {
        "vid": "33E4",
        "pid": "5908",
        "wired_pid": "5917"
      },
      {
        "vid": "33E4",
        "pid": "7204",
        "wired_pid": "7203"
      },
      {
        "vid": "33E4",
        "pid": "7708",
        "wired_pid": "7717"
      },
      {
        "vid": "33E4",
        "pid": "5804",
        "wired_pid": "5803"
      },
      {
        "vid": "33E4",
        "pid": "5404",
        "wired_pid": "5403"
      },
      {
        "vid": "33E4",
        "pid": "5704",
        "wired_pid": "5703"
      },
      {
        "vid": "33E4",
        "pid": "5904",
        "wired_pid": "5903"
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": null,
      "feature_len": 65,
      "ignore_usage_page": true,
      "require_pid_hit": true
    },
    "request": {
      "transport": "feature_write_read",
      "payload_hex": "00 00 02 8F 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00",
      "out_len": 65,
      "in_len": 65,
      "report_id": 0
    },
    "response": {
      "min_length": 7,
      "markers": [
        [
          0,
          161
        ],
        [
          1,
          2
        ],
        [
          2,
          143
        ]
      ],
      "level": {
        "offset": 6
      },
      "charging": {
        "offset": 5,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "ninjutso-sora-v2": {
    "schema_version": 1,
    "label": "Ninjutso Sora V2 鼠标",
    "vendor": "Ninjutso",
    "keywords": "Ninjutso Sora V2 鼠标",
    "notes": "mouse_tray/2.4G-viewer：1915:AE1C，FFA0:0001，Feature 5/32B，05 15 00 00 01；level[9]、charging[10]。",
    "known_ids": [
      {
        "vid": "1915",
        "pid": "AE1C",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFA0",
      "usage": "0x0001",
      "require_pid_hit": false
    },
    "request": {
      "transport": "feature_write_read",
      "payload_hex": "05 15 00 00 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00",
      "out_len": 32,
      "in_len": 32,
      "report_id": 5
    },
    "response": {
      "min_length": 13,
      "markers": [
        [
          0,
          5
        ]
      ],
      "level": {
        "offset": 9
      },
      "charging": {
        "offset": 10,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "flydigi-apex5": {
    "schema_version": 1,
    "label": "飞智 Flydigi APEX5 手柄",
    "vendor": "飞智 Flydigi",
    "keywords": "Flydigi APEX5 手柄",
    "notes": "myfinder：37D7:2501，FFA0:0001，report 3/4 32B；03 5A A5 01 02 03；level[12] 0..5×20。",
    "known_ids": [
      {
        "vid": "37D7",
        "pid": "2501",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFA0",
      "usage": "0x0001",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "03 5A A5 01 02 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00",
      "out_len": 32,
      "in_len": 32,
      "report_id": 3,
      "read_matching": true
    },
    "response": {
      "exact_length": 32,
      "markers": [
        [
          0,
          4
        ],
        [
          1,
          90
        ],
        [
          2,
          165
        ],
        [
          3,
          1
        ],
        [
          4,
          1
        ],
        [
          5,
          0
        ],
        [
          6,
          128
        ]
      ],
      "status": {
        "offset": 7,
        "ok_values": [
          1,
          2
        ]
      },
      "level": {
        "offset": 12,
        "min": 0,
        "max": 5,
        "scale": 20,
        "mask": 15
      },
      "charging": {
        "offset": 12,
        "when_charging": 16,
        "mask": 240
      }
    },
    "sampling": {
      "attempts": 2,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "flydigi-new": {
    "schema_version": 1,
    "label": "飞智 Flydigi 新架构手柄",
    "vendor": "飞智 Flydigi",
    "keywords": "Flydigi 手柄 新架构手柄",
    "notes": "myfinder：37D7:2502，FFA0:0001，00 5A A5 01 02 00；level[11] 0..10×10。",
    "known_ids": [
      {
        "vid": "37D7",
        "pid": "2502",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFA0",
      "usage": "0x0001",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 5A A5 01 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00",
      "out_len": 32,
      "in_len": 32,
      "read_matching": true
    },
    "response": {
      "min_length": 31,
      "markers": [
        [
          0,
          90
        ],
        [
          1,
          165
        ],
        [
          2,
          1
        ]
      ],
      "level": {
        "offset": 11,
        "min": 0,
        "max": 10,
        "scale": 10,
        "mask": 15
      },
      "charging": {
        "offset": 11,
        "when_charging": 16,
        "mask": 240
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "flydigi-vader4": {
    "schema_version": 1,
    "label": "飞智 Flydigi Vader 4 Pro 手柄",
    "vendor": "飞智 Flydigi",
    "keywords": "Flydigi Vader 4 Pro 手柄",
    "notes": "hurttttr：04B4:2412，FFA0:0001，interface 2，05 EC；level[11] 0..5×20；2.4G 充电位固件不下发。",
    "known_ids": [
      {
        "vid": "04B4",
        "pid": "2412",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFA0",
      "usage": "0x0001",
      "interface": 2,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "05 EC 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00",
      "out_len": 32,
      "in_len": 32,
      "report_id": 5,
      "read_matching": true
    },
    "response": {
      "min_length": 16,
      "markers": [
        [
          0,
          4
        ],
        [
          1,
          255
        ],
        [
          15,
          236
        ]
      ],
      "level": {
        "offset": 11,
        "min": 0,
        "max": 5,
        "scale": 20,
        "mask": 15
      },
      "charging": {
        "offset": 11,
        "when_charging": 16,
        "mask": 240
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "keychron-m-series": {
    "schema_version": 1,
    "label": "渴创 Keychron M1/M2/M3/M6/M7 鼠标",
    "vendor": "渴创 Keychron",
    "keywords": "Keychron M1 M2 M3 M6 M7 鼠标",
    "notes": "keychron.cpp/ferloom：3434:D030/D031，usage 0x008C/0x0001，Feature 51/1001B，level[11]、charging[12]。",
    "known_ids": [
      {
        "vid": "3434",
        "pid": "D030",
        "wired_pid": ""
      },
      {
        "vid": "3434",
        "pid": "D031",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0x008C",
      "usage": "0x0001",
      "interface": 1,
      "feature_len": 1001,
      "require_pid_hit": false
    },
    "request": {
      "transport": "feature_write_read",
      "payload_hex": "51 06",
      "out_len": 1001,
      "in_len": 1001,
      "report_id": 81
    },
    "response": {
      "min_length": 13,
      "markers": [
        [
          0,
          81
        ],
        [
          1,
          6
        ],
        [
          2,
          1
        ],
        [
          4,
          52
        ],
        [
          5,
          52
        ]
      ],
      "status": {
        "offset": 2,
        "ok_values": [
          1
        ]
      },
      "level": {
        "offset": 11
      },
      "charging": {
        "offset": 12,
        "when_charging": 1
      }
    },
    "sampling": {
      "attempts": 5,
      "stable_count": 1,
      "request_timeout_ms": 600,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "keychron-m4": {
    "schema_version": 1,
    "label": "渴创 Keychron M4 鼠标",
    "vendor": "渴创 Keychron",
    "keywords": "Keychron M4 鼠标",
    "notes": "keychron.cpp：3434:D043/D053/D048，64B output/input + 0xA1−sum；level[11]、charging[10]。",
    "known_ids": [
      {
        "vid": "3434",
        "pid": "D043",
        "wired_pid": ""
      },
      {
        "vid": "3434",
        "pid": "D053",
        "wired_pid": ""
      },
      {
        "vid": "3434",
        "pid": "D048",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 01 00 81 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 1E",
      "out_len": 65,
      "in_len": 65,
      "read_matching": true,
      "checksum": {
        "algo": "sum8",
        "start": 1,
        "end": 64,
        "target": 64,
        "out": "base_minus",
        "base": 161
      }
    },
    "response": {
      "min_length": 13,
      "markers": [
        [
          0,
          1
        ],
        [
          3,
          1
        ],
        [
          6,
          52
        ],
        [
          7,
          52
        ]
      ],
      "level": {
        "offset": 11,
        "min": 1,
        "max": 100
      },
      "charging": {
        "offset": 10,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 2,
      "stable_count": 1,
      "request_timeout_ms": 600,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "keychron-8k-receiver": {
    "schema_version": 1,
    "label": "渴创 Keychron 8K 接收器鼠标",
    "vendor": "渴创 Keychron",
    "keywords": "Keychron 8K 接收器 鼠标 接收器鼠标",
    "notes": "keychron.cpp：3434:D028/D038，00 41 00 81 01…DE；level[11]、charging[10]=1。",
    "known_ids": [
      {
        "vid": "3434",
        "pid": "D028",
        "wired_pid": ""
      },
      {
        "vid": "3434",
        "pid": "D038",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 41 00 81 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 DE",
      "out_len": 65,
      "in_len": 65,
      "read_matching": true,
      "checksum": {
        "algo": "sum8",
        "start": 1,
        "end": 64,
        "target": 64,
        "out": "base_minus",
        "base": 161
      }
    },
    "response": {
      "min_length": 13,
      "markers": [
        [
          0,
          65
        ],
        [
          3,
          1
        ]
      ],
      "level": {
        "offset": 11,
        "min": 1,
        "max": 100
      },
      "charging": {
        "offset": 10,
        "when_charging": 1
      }
    },
    "sampling": {
      "attempts": 2,
      "stable_count": 1,
      "request_timeout_ms": 600,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "rongyuan-keyboard": {
    "schema_version": 1,
    "label": "容源 Rongyuan 2.4G 键盘接收器",
    "vendor": "容源 Rongyuan",
    "keywords": "Rongyuan 容源 接收器 键盘 2.4G 键盘接收器",
    "notes": "ferloom：3151 系列，FFFF:0002，interface 2，Feature 0/65B，F7；键盘槽 level[2]、在线[4]=0。",
    "known_ids": [
      {
        "vid": "3151",
        "pid": "4014",
        "wired_pid": ""
      },
      {
        "vid": "3151",
        "pid": "4017",
        "wired_pid": ""
      },
      {
        "vid": "3151",
        "pid": "4021",
        "wired_pid": ""
      },
      {
        "vid": "3151",
        "pid": "4027",
        "wired_pid": ""
      },
      {
        "vid": "3151",
        "pid": "4038",
        "wired_pid": ""
      },
      {
        "vid": "3151",
        "pid": "5006",
        "wired_pid": ""
      },
      {
        "vid": "3151",
        "pid": "5008",
        "wired_pid": ""
      },
      {
        "vid": "3151",
        "pid": "5037",
        "wired_pid": ""
      },
      {
        "vid": "3151",
        "pid": "5038",
        "wired_pid": ""
      },
      {
        "vid": "3151",
        "pid": "503D",
        "wired_pid": ""
      },
      {
        "vid": "3151",
        "pid": "503F",
        "wired_pid": ""
      },
      {
        "vid": "3151",
        "pid": "5040",
        "wired_pid": ""
      },
      {
        "vid": "3151",
        "pid": "5042",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFFF",
      "usage": "0x0002",
      "interface": 2,
      "feature_len": 65,
      "require_pid_hit": false
    },
    "request": {
      "transport": "feature_write_read",
      "payload_hex": "00 F7 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00",
      "out_len": 65,
      "in_len": 65,
      "report_id": 0
    },
    "response": {
      "min_length": 7,
      "markers": [
        [
          0,
          0
        ],
        [
          7,
          1
        ],
        [
          4,
          0
        ]
      ],
      "level": {
        "offset": 2,
        "min": 1,
        "max": 100
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "astro-a50-gen5": {
    "schema_version": 1,
    "label": "ASTRO A50 Gen 5 耳机",
    "vendor": "ASTRO",
    "keywords": "Astro A50 Gen 5 耳机",
    "notes": "myfinder：046D:0B1C，FF32:0074，report 02 cmd 06；level[6]、charging[8]。",
    "known_ids": [
      {
        "vid": "046D",
        "pid": "0B1C",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF32",
      "usage": "0x0074",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "02 0C 03 00 06 0C 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00",
      "out_len": 64,
      "in_len": 64,
      "report_id": 2,
      "read_matching": true
    },
    "response": {
      "min_length": 9,
      "markers": [
        [
          0,
          2
        ],
        [
          1,
          12
        ],
        [
          4,
          6
        ]
      ],
      "level": {
        "offset": 6
      },
      "charging": {
        "offset": 8,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 2,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "hyperx-pulsefire-haste2": {
    "schema_version": 1,
    "label": "极度未知 HyperX Pulsefire Haste 2 无线鼠标",
    "vendor": "极度未知 HyperX",
    "keywords": "HyperX Pulsefire Haste 2 无线鼠标",
    "notes": "myfinder：03F0:0D97/0F98，interface 2，FF00:0001，50 02；level[2]、charging[3]。",
    "known_ids": [
      {
        "vid": "03F0",
        "pid": "0D97",
        "wired_pid": ""
      },
      {
        "vid": "03F0",
        "pid": "0F98",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": "0x0001",
      "interface": 2,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "50 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00",
      "out_len": 64,
      "in_len": 64,
      "report_id": 80,
      "read_matching": true
    },
    "response": {
      "min_length": 10,
      "markers": [
        [
          0,
          81
        ],
        [
          1,
          2
        ]
      ],
      "level": {
        "offset": 2
      },
      "charging": {
        "offset": 3,
        "when_charging": 1
      }
    },
    "sampling": {
      "attempts": 2,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "sony-inzone-h5": {
    "schema_version": 1,
    "label": "索尼 Sony INZONE H5 耳机",
    "vendor": "索尼 Sony",
    "keywords": "Sony INZONE H5 耳机",
    "notes": "myfinder：054C:0EBF，FF04:0002，64B report 02；level[14]、charging[13]。",
    "known_ids": [
      {
        "vid": "054C",
        "pid": "0EBF",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF04",
      "usage": "0x0002",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "02 0C 01 00 FC 08 96 C3 41 04 01 02 00 A1 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00",
      "out_len": 64,
      "in_len": 64,
      "report_id": 2,
      "read_matching": true
    },
    "response": {
      "min_length": 14,
      "markers": [
        [
          0,
          2
        ],
        [
          2,
          4
        ],
        [
          3,
          255
        ],
        [
          5,
          0
        ],
        [
          6,
          150
        ],
        [
          7,
          195
        ],
        [
          8,
          20
        ],
        [
          9,
          4
        ],
        [
          11,
          2
        ],
        [
          12,
          0
        ]
      ],
      "level": {
        "offset": 14
      },
      "charging": {
        "offset": 13,
        "when_charging": 1
      }
    },
    "sampling": {
      "attempts": 2,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "eweadn-s9-089d": {
    "schema_version": 1,
    "label": "前行者 EWEADN S9 鼠标",
    "vendor": "前行者 EWEADN",
    "keywords": "EWEADN S9 鼠标 前行者",
    "notes": "myfinder：089D:062F，FF01:0010，33B 00 30；level[13]、charging[12]。",
    "known_ids": [
      {
        "vid": "089D",
        "pid": "062F",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF01",
      "usage": "0x0010",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 30 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00",
      "out_len": 33,
      "in_len": 33,
      "report_id": 0,
      "read_matching": true
    },
    "response": {
      "min_length": 15,
      "markers": [
        [
          0,
          48
        ],
        [
          1,
          0
        ],
        [
          4,
          77
        ],
        [
          5,
          49
        ],
        [
          6,
          53
        ],
        [
          7,
          54
        ]
      ],
      "level": {
        "offset": 13
      },
      "charging": {
        "offset": 12,
        "when_charging": 1
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "kysona-m600": {
    "schema_version": 1,
    "label": "Kysona M600 鼠标",
    "vendor": "Kysona",
    "keywords": "Kysona M600 Mercury 鼠标",
    "notes": "artnway/ferloom：3554:F57C/F57D/F5D5，FF02:0002，interface 1，17B，0x55−sum；level[6]、charging[7]。",
    "known_ids": [
      {
        "vid": "3554",
        "pid": "F57C",
        "wired_pid": "F57D"
      },
      {
        "vid": "3554",
        "pid": "F5D5",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF02",
      "usage": "0x0002",
      "interface": 1,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "08 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00",
      "out_len": 17,
      "in_len": 17,
      "read_matching": true,
      "checksum": {
        "algo": "sum8",
        "start": 0,
        "end": 16,
        "target": 16,
        "out": "base_minus",
        "base": 85,
        "zero_target": true
      }
    },
    "response": {
      "min_length": 7,
      "markers": [
        [
          0,
          8
        ],
        [
          1,
          4
        ]
      ],
      "level": {
        "offset": 6
      },
      "charging": {
        "offset": 7,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "aula-l9": {
    "schema_version": 1,
    "label": "狼蛛 AULA L9 Ultra 鼠标",
    "vendor": "狼蛛 AULA",
    "keywords": "AULA L9 Ultra 鼠标",
    "notes": "LargeAkon/vigil-watt：38A6:28FD，FFA8，0B 81 → 0A 81；level[18]、charging[17]。",
    "known_ids": [
      {
        "vid": "38A6",
        "pid": "28FD",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFA8",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "0B 81 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00",
      "out_len": 64,
      "in_len": 64,
      "read_matching": true
    },
    "response": {
      "min_length": 19,
      "markers": [
        [
          0,
          10
        ],
        [
          1,
          129
        ]
      ],
      "level": {
        "offset": 18
      },
      "charging": {
        "offset": 17,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "ajazz-ak870": {
    "schema_version": 1,
    "label": "黑爵 AJAZZ AK870 键盘",
    "vendor": "黑爵 AJAZZ",
    "keywords": "AJAZZ AK870 键盘 黑爵",
    "notes": "LargeAkon：3151:4011，FFFF:0002，Feature 0/65B，F7 08；level[2]、在线[4]=0，无充电位。",
    "known_ids": [
      {
        "vid": "3151",
        "pid": "4011",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFFF",
      "usage": "0x0002",
      "feature_len": 65,
      "require_pid_hit": false
    },
    "request": {
      "transport": "feature_write_read",
      "payload_hex": "00 F7 00 00 00 00 00 00 08 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00",
      "out_len": 65,
      "in_len": 65,
      "report_id": 0
    },
    "response": {
      "min_length": 5,
      "markers": [
        [
          0,
          0
        ],
        [
          4,
          0
        ]
      ],
      "status": {
        "offset": 1,
        "ok_values": [
          0,
          1
        ]
      },
      "level": {
        "offset": 2
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "rapoo-mt560-passive": {
    "schema_version": 1,
    "label": "雷柏 Rapoo MT560 鼠标",
    "vendor": "雷柏 Rapoo",
    "keywords": "Rapoo MT560 鼠标",
    "notes": "myfinder：24AE:1872，FF00:000E，被动 BB B0，187B；level[6]、充电未确认。",
    "known_ids": [
      {
        "vid": "24AE",
        "pid": "1872",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": "0x000E",
      "require_pid_hit": false
    },
    "request": {
      "transport": "passive",
      "in_len": 187
    },
    "passive": {
      "stale_after_s": 15,
      "initial_wait_ms": 2500
    },
    "response": {
      "min_length": 7,
      "markers": [
        [
          0,
          187
        ],
        [
          1,
          176
        ]
      ],
      "status": {
        "offset": 5,
        "ok_values": [
          1
        ]
      },
      "level": {
        "offset": 6
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "mchose-l7-pro-passive": {
    "schema_version": 1,
    "label": "迈丛 MCHOSE L7 Pro 鼠标",
    "vendor": "迈丛 MCHOSE",
    "keywords": "MCHOSE L7 Pro 鼠标 迈丛",
    "notes": "Fan4Metal：5253:1020/00B0，FF01:0001，被动 0x13/0x1D，整帧取反；level[5]、wired[4]。",
    "known_ids": [
      {
        "vid": "5253",
        "pid": "1020",
        "wired_pid": "00B0"
      }
    ],
    "match": {
      "usage_page": "0xFF01",
      "usage": "0x0001",
      "require_pid_hit": false
    },
    "request": {
      "transport": "passive",
      "in_len": 64
    },
    "passive": {
      "stale_after_s": 15,
      "initial_wait_ms": 2500
    },
    "response": {
      "transform": "invert",
      "min_length": 6,
      "markers": [
        [
          0,
          236
        ],
        [
          1,
          226
        ]
      ],
      "level": {
        "offset": 5
      },
      "charging": {
        "offset": 4,
        "nonzero": true
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "aula-g9-passive": {
    "schema_version": 1,
    "label": "狼蛛 AULA G9 耳机",
    "vendor": "狼蛛 AULA",
    "keywords": "AULA G9 耳机",
    "notes": "LargeAkon：38A6:0030，FFAB，被动 AB 47 4D 86；level[6]、charging[7]=2。",
    "known_ids": [
      {
        "vid": "38A6",
        "pid": "0030",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFFAB",
      "usage": null,
      "require_pid_hit": false
    },
    "request": {
      "transport": "passive",
      "in_len": 64
    },
    "passive": {
      "stale_after_s": 15,
      "initial_wait_ms": 2500
    },
    "response": {
      "min_length": 8,
      "markers": [
        [
          0,
          171
        ],
        [
          1,
          71
        ],
        [
          2,
          77
        ],
        [
          3,
          134
        ]
      ],
      "level": {
        "offset": 6
      },
      "charging": {
        "offset": 7,
        "when_charging": 2
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "hyperx-cf2-passive": {
    "schema_version": 1,
    "label": "极度未知 HyperX Cloud Flight 2 耳机",
    "vendor": "极度未知 HyperX",
    "keywords": "HyperX Cloud Flight 2 耳机",
    "notes": "LargeAkon：03F0:0AC1，FF90:FF00，被动 FB 13；level[3]，充电需 FB 09 另帧。",
    "known_ids": [
      {
        "vid": "03F0",
        "pid": "0AC1",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF90",
      "usage": "0xFF00",
      "require_pid_hit": false
    },
    "request": {
      "transport": "passive",
      "in_len": 64
    },
    "passive": {
      "stale_after_s": 15,
      "initial_wait_ms": 3000
    },
    "response": {
      "min_length": 4,
      "markers": [
        [
          0,
          251
        ],
        [
          1,
          19
        ]
      ],
      "level": {
        "offset": 3
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1200,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "asus-rog-cetra": {
    "schema_version": 1,
    "label": "华硕 ROG Cetra SpeedNova 耳机",
    "vendor": "华硕 ROG",
    "keywords": "ROG Cetra SpeedNova 耳机 华硕",
    "notes": "g-helper：0B05:1AD3，MI_03，FF00:0001，CC 12 07；level[6] 左耳（右耳/仓另字节）。",
    "known_ids": [
      {
        "vid": "0B05",
        "pid": "1AD3",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF00",
      "usage": "0x0001",
      "interface": 3,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "write_mode": "set_output_report",
      "payload_hex": "CC 12 07",
      "out_len": 64,
      "in_len": 64
    },
    "response": {
      "min_length": 9,
      "markers": [
        [
          0,
          204
        ],
        [
          1,
          18
        ],
        [
          2,
          7
        ]
      ],
      "level": {
        "offset": 6
      }
    },
    "sampling": {
      "attempts": 3,
      "stable_count": 1,
      "request_timeout_ms": 1000,
      "total_timeout_s": 4.5,
      "pick": "last"
    }
  },
  "mchose-v9-pro2-headset": {
    "schema_version": 1,
    "label": "迈丛 V9 Pro 2 耳机",
    "keywords": "V9 Pro 2 V9PRO2 MC 6011 FF07 迈丛 MCHOSE 耳机",
    "vendor": "迈丛 MCHOSE",
    "notes": "来源 M HUB MC 协议（myfinder-device-battery mchose_v9pro2）：FF07:0222、interface 3、report 0x55；请求 AA 01 00 01 00 01 0B 00 补零至 64；回包 55 AA 01…，[8] awake、[9] 低 7 位电量 / 最高位充电。",
    "known_ids": [
      {
        "vid": "3837",
        "pid": "6011",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF07",
      "usage": "0x0222",
      "interface": 3,
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 AA 01 00 01 00 01 0B 00",
      "out_len": 64,
      "in_len": 64,
      "report_id": 85
    },
    "response": {
      "markers": [
        [
          0,
          85
        ],
        [
          1,
          170
        ],
        [
          2,
          1
        ],
        [
          5,
          0
        ],
        [
          6,
          1
        ],
        [
          7,
          11
        ]
      ],
      "exact_length": 64,
      "min_length": 64,
      "status": {
        "offset": 8,
        "ok_values": [
          1
        ]
      },
      "level": {
        "offset": 9,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0,
        "mask": 127
      },
      "charging": {
        "offset": 9,
        "nonzero": true,
        "mask": 128
      }
    }
  },
  "mchose-r9-headset": {
    "schema_version": 1,
    "label": "迈丛 R9 耳机",
    "keywords": "R9 MC 6013 FF07 迈丛 MCHOSE 耳机",
    "vendor": "迈丛 MCHOSE",
    "notes": "来源 M HUB MC 协议（myfinder-device-battery mchose_v9pro2）：R9 3837:6013 同帧，FF07:0222、report 0x55；interface 未确认，不按接口过滤。",
    "known_ids": [
      {
        "vid": "3837",
        "pid": "6013",
        "wired_pid": ""
      }
    ],
    "match": {
      "usage_page": "0xFF07",
      "usage": "0x0222",
      "require_pid_hit": false
    },
    "request": {
      "transport": "write_read",
      "payload_hex": "00 AA 01 00 01 00 01 0B 00",
      "out_len": 64,
      "in_len": 64,
      "report_id": 85
    },
    "response": {
      "markers": [
        [
          0,
          85
        ],
        [
          1,
          170
        ],
        [
          2,
          1
        ],
        [
          5,
          0
        ],
        [
          6,
          1
        ],
        [
          7,
          11
        ]
      ],
      "exact_length": 64,
      "min_length": 64,
      "status": {
        "offset": 8,
        "ok_values": [
          1
        ]
      },
      "level": {
        "offset": 9,
        "encoding": "u8",
        "min": 0,
        "max": 100,
        "scale": 1,
        "add": 0,
        "mask": 127
      },
      "charging": {
        "offset": 9,
        "nonzero": true,
        "mask": 128
      }
    }
  }
}"#;

// ---------- 归一化后的结构 ----------

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Profile {
    pub id: String,
    pub label: String,
    /// 厂家分组（菜单用），缺省"其他"。
    pub vendor: String,
    /// 搜索别名：型号/俗称（如「凌豹 K98」），空格分隔，菜单搜索命中。
    pub keywords: String,
    /// 会话 ACK 型号码（Compx 公版接收器）。同 VID/PID 被多个品牌共用时按它分流；
    /// 读法与分流依据见 vigil-watt `CompxSessionAck`（ACK 第 9/10 字节）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_code: Option<u16>,
    pub schema_version: u32,
    pub source: String,
    pub enabled: bool,
    pub notes: String,
    pub known_ids: Vec<KnownId>,
    /// 被动监听参数；仅 `transport = passive` 时使用。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passive: Option<PassiveRule>,
    /// JSON 里叫 `match`（Rust 关键字），只在本字段名上做 serde 重命名。
    #[serde(rename = "match")]
    pub match_rule: MatchRule,
    pub request: Request,
    pub response: Response,
    pub sampling: Sampling,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownId {
    pub vid: String,
    pub pid: String,
    pub wired_pid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatchRule {
    pub usage_page: u16,
    pub usage: Option<u16>,
    /// 可选 USB 接口号（路径里的 `mi_XX`）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface: Option<u8>,
    /// 可选 HID 集合号（路径里的 `colXX`）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection: Option<u8>,
    /// 可选 FeatureReportByteLength 精确门禁（G-Wolves 等按 feature 长度选集合）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_len: Option<u16>,
    /// G-Wolves 这类不按 usage page 匹配的族设为 true。
    pub ignore_usage_page: bool,
    /// 命名遗留：true 时跳过「精确 PID 优先」，直接用同 VID 的所有接口（见 hid 模块）。
    pub require_pid_hit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Request {
    pub transport: Transport,
    pub payload: Vec<u8>,
    pub out_len: Option<u16>,
    pub in_len: Option<u16>,
    pub report_id: Option<u8>,
    /// Opt-in response filtering within one read deadline.
    pub read_matching: bool,
    /// `write_file`（WriteFile）或 `set_output_report`（HidD_SetOutputReport）。
    pub write_mode: WriteMode,
    /// 请求帧整包变换；目前只有 `invert`。
    pub transform: ByteTransform,
    /// 请求帧校验和写入；没有则跳过。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<ChecksumSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
// 变体名对应 profile JSON 里固定的 transport 取值，改名会破坏外置协议；
// 三个变体都以 Read 结尾是协议本身的名字，不是命名事故。
#[allow(clippy::enum_variant_names)]
pub enum Transport {
    #[serde(rename = "write_read")]
    WriteRead,
    #[serde(rename = "feature_read")]
    FeatureRead,
    /// SetFeature（写查询帧）→ 短暂等待 → GetFeature（读应答）。
    /// 凌豹 M5PRO / EWEADN S9 Pro 这类「feature 问答」设备用。
    #[serde(rename = "feature_write_read")]
    FeatureWriteRead,
    /// 只监听 Input Report，从不写；`passive` 参数控制缓存与首帧等待。
    #[serde(rename = "passive")]
    Passive,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Response {
    /// All byte markers must match; absent for legacy profiles.
    pub markers: Vec<(u8, u8)>,
    pub exact_length: Option<usize>,
    pub min_length: usize,
    /// 回包整包变换（在长度/markers/status 之前）。
    pub transform: ByteTransform,
    /// 回包校验和；不配置则不做校验。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<ChecksumSpec>,
    pub status: StatusRule,
    pub level: LevelRule,
    pub charging: ChargingRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatusRule {
    pub offset: Option<u8>,
    pub ok_values: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LevelRule {
    pub offset: u8,
    pub encoding: LevelEncoding,
    pub min: i32,
    pub max: i32,
    pub scale: f64,
    pub add: f64,
    /// 可选位掩码（如 `0x7F`）：取值先与掩码按位与再进 min/max——
    /// DARKRIM/VGN kc2 等把充电位与百分比挤在同一字节。
    pub mask: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LevelEncoding {
    #[serde(rename = "u8")]
    U8,
    #[serde(rename = "u16le")]
    U16Le,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChargingRule {
    pub offset: Option<u8>,
    pub nonzero: bool,
    pub when_charging: Option<i32>,
    /// 可选位掩码（如 `0x80`）：字节先按位与再判 nonzero/when_charging。
    pub mask: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub enum WriteMode {
    #[serde(rename = "write_file")]
    #[default]
    WriteFile,
    /// `HidD_SetOutputReport`，用于必须走控制输出的设备（如 ROG PELTA）。
    #[serde(rename = "set_output_report")]
    SetOutputReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub enum ByteTransform {
    #[serde(rename = "none")]
    #[default]
    None,
    /// 整包按位取反（MCHOSE A7 等）。
    #[serde(rename = "invert")]
    Invert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ChecksumAlgo {
    #[serde(rename = "sum8")]
    Sum8,
    #[serde(rename = "xor8")]
    Xor8,
    #[serde(rename = "crc16_ccitt_false")]
    Crc16CcittFalse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub enum ChecksumEndian {
    #[serde(rename = "little")]
    #[default]
    Little,
    #[serde(rename = "big")]
    Big,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub enum ChecksumOut {
    #[serde(rename = "raw")]
    #[default]
    Raw,
    /// 取反：8 位 ^0xFF，16 位 ^0xFFFF。
    #[serde(rename = "not")]
    Not,
    /// `base - raw`（按位宽 wrapping）。
    #[serde(rename = "base_minus")]
    BaseMinus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChecksumSpec {
    pub algo: ChecksumAlgo,
    /// 计算范围 `[start, end)`，按含 report ID 的缓冲计。
    pub start: u16,
    pub end: u16,
    /// 校验值写入/读取的起始偏移。
    pub target: u16,
    pub endian: ChecksumEndian,
    pub out: ChecksumOut,
    /// `out = base_minus` 时的基数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<u16>,
    /// 计算前先把 target 字节清零。
    pub zero_target: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PassiveRule {
    /// 最后一次有效帧超过该秒数视为休眠；0 = 永不过期。
    pub stale_after_s: f64,
    /// 首帧最多等待的毫秒数。
    pub initial_wait_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sampling {
    pub attempts: u8,
    pub stable_count: u8,
    pub request_timeout_ms: u64,
    pub total_timeout_s: f64,
    pub pick: Pick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Pick {
    #[serde(rename = "mode_nonzero")]
    ModeNonzero,
    #[serde(rename = "last")]
    Last,
    #[serde(rename = "median")]
    Median,
}

/// 单个坏文件的错误（对应 V16 的 `{"file": ..., "error": ...}`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileError {
    pub file: String,
    pub error: String,
}

impl ProfileError {
    pub fn new(file: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            error: error.into(),
        }
    }
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.file, self.error)
    }
}

// ---------- 解析工具（对应 profiles.py 的 _int / _hex_bytes / _offset / _check） ----------

/// Python 的真值判断（`x or default` 用）。
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

/// `str(x)`：字符串原样，其余按 Python 的文本形式（数组/对象退化为 JSON 文本）。
fn as_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::Null => "None".to_string(),
        other => other.to_string(),
    }
}

/// `str(obj.get(key) or "")`。
fn text_field(object: &Value, key: &str) -> String {
    match object.get(key) {
        Some(value) if truthy(value) => as_text(value),
        _ => String::new(),
    }
}

/// `obj.get(key)`（obj 非对象或缺失时为 None）。
fn sub<'a>(object: Option<&'a Value>, key: &str) -> Option<&'a Value> {
    object.and_then(|value| value.get(key))
}

/// Python 的 `int(text, 0)`：支持 `0x` / `0o` / `0b` 前缀与数字间下划线。
fn parse_int_text(text: &str) -> Option<i64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let cleaned: String = trimmed.chars().filter(|c| *c != '_').collect();
    let (negative, body) = match cleaned.as_bytes().first() {
        Some(b'-') => (true, &cleaned[1..]),
        Some(b'+') => (false, &cleaned[1..]),
        _ => (false, cleaned.as_str()),
    };
    let lowered = body.to_ascii_lowercase();
    let (digits, radix) = if let Some(rest) = lowered.strip_prefix("0x") {
        (rest, 16)
    } else if let Some(rest) = lowered.strip_prefix("0o") {
        (rest, 8)
    } else if let Some(rest) = lowered.strip_prefix("0b") {
        (rest, 2)
    } else {
        (lowered.as_str(), 10)
    };
    // 注：Python 的 base=0 会拒绝 "010"，这里按十进制接受（见文件末尾差异说明）。
    let magnitude = i64::from_str_radix(digits, radix).ok()?;
    Some(if negative { -magnitude } else { magnitude })
}

/// Python 的 `int(value)`（转换失败返回 None）。浮点向零截断，超大值饱和。
fn as_int(value: &Value) -> Option<i64> {
    match value {
        Value::Null => None,
        Value::Bool(b) => Some(i64::from(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(i)
            } else if let Some(u) = n.as_u64() {
                i64::try_from(u).ok()
            } else {
                n.as_f64().map(|f| f as i64)
            }
        }
        Value::String(s) => parse_int_text(s),
        _ => None,
    }
}

/// Python 的 `_int(value, default)`：None / 转换失败都回落 default。
fn int_of(value: Option<&Value>, default: Option<i64>) -> Option<i64> {
    match value {
        None | Some(Value::Null) => default,
        Some(value) => as_int(value).or(default),
    }
}

/// Python 的 `float(value)`。
fn as_float(value: &Value) -> Option<f64> {
    match value {
        Value::Null => None,
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// Python 的 `float(value or default)`：缺失/假值回落 default，解析失败报错。
fn float_or(value: Option<&Value>, default: f64, name: &str) -> Result<f64, String> {
    match value {
        None => Ok(default),
        Some(value) if !truthy(value) => Ok(default),
        Some(value) => as_float(value).ok_or_else(|| format!("{name} 不是数字")),
    }
}

/// 超出 i32 的值饱和（V16 是无界整数，这里只能夹住，见差异说明）。
fn clamp_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// `'0x08 04' / '08,04' / '0804' → [0x08, 0x04]`；非法返回错误文本。
fn hex_bytes(value: Option<&Value>) -> Result<Vec<u8>, String> {
    let Some(Value::String(text)) = value else {
        return Err("payload_hex 必须是字符串".to_string());
    };
    let without_prefix = text.replace("0x", "").replace("0X", "");
    let cleaned: String = without_prefix
        .chars()
        .filter(|c| !(c.is_whitespace() || matches!(c, ',' | ';' | ':' | '_' | '-')))
        .collect();
    if cleaned.is_empty() {
        return Err("payload_hex 为空".to_string());
    }
    if !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("payload_hex 含非十六进制字符".to_string());
    }
    if !cleaned.len().is_multiple_of(2) {
        return Err("payload_hex 长度必须是偶数".to_string());
    }
    let mut payload = Vec::with_capacity(cleaned.len() / 2);
    let mut digits = cleaned.chars();
    while let (Some(high), Some(low)) = (digits.next(), digits.next()) {
        let high = high
            .to_digit(16)
            .ok_or_else(|| "payload_hex 含非十六进制字符".to_string())?;
        let low = low
            .to_digit(16)
            .ok_or_else(|| "payload_hex 含非十六进制字符".to_string())?;
        payload.push(((high << 4) | low) as u8);
    }
    if !(1..=1024).contains(&payload.len()) {
        return Err("payload_hex 必须是 1..1024 字节".to_string());
    }
    Ok(payload)
}

/// Python 的 `_offset`：0..255，解析失败报「缺失」。
fn offset_of(value: Option<&Value>, name: &str, default: Option<i64>) -> Result<u8, String> {
    let number = int_of(value, default).ok_or_else(|| format!("{name} 缺失"))?;
    if !(0..=255).contains(&number) {
        return Err(format!("{name} 必须在 0..255"));
    }
    Ok(number as u8)
}

/// `[a-z0-9][a-z0-9_.-]{0,63}`（整串匹配，含 `\Z` 语义）。
fn valid_id(id: &str) -> bool {
    let count = id.chars().count();
    if count == 0 || count > 64 {
        return false;
    }
    let mut chars = id.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    first_ok
        && chars
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-'))
}

// ---------- 归一化 ----------

/// 把 JSON 归一化成读取端直接可用的结构（对应 `profiles.normalize`）。
fn parse_transform(raw: Option<&Value>, field: &str) -> Result<ByteTransform, String> {
    let text = match raw {
        None | Some(Value::Null) => "none".to_string(),
        Some(value) if truthy(value) => as_text(value),
        _ => return Err(format!("{field} 只能是 none / invert")),
    };
    match text.trim().to_lowercase().as_str() {
        "none" => Ok(ByteTransform::None),
        "invert" => Ok(ByteTransform::Invert),
        _ => Err(format!("{field} 只能是 none / invert")),
    }
}

fn checksum_spec(raw: Option<&Value>, field: &str) -> Result<Option<ChecksumSpec>, String> {
    let Some(value) = raw else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    if !value.is_object() {
        return Err(format!("{field} 必须是对象"));
    }

    let algo_text = match value.get("algo") {
        Some(value) if truthy(value) => as_text(value),
        _ => "sum8".to_string(),
    };
    let algo = match algo_text.trim().to_lowercase().as_str() {
        "sum8" => ChecksumAlgo::Sum8,
        "xor8" => ChecksumAlgo::Xor8,
        "crc16_ccitt_false" => ChecksumAlgo::Crc16CcittFalse,
        _ => {
            return Err(format!(
                "{field}.algo 只能是 sum8 / xor8 / crc16_ccitt_false"
            ));
        }
    };
    let width = match algo {
        ChecksumAlgo::Crc16CcittFalse => 2,
        _ => 1,
    };

    let start = int_of(value.get("start"), Some(0)).unwrap_or(0);
    let end = int_of(value.get("end"), None).ok_or_else(|| format!("{field}.end 必填"))?;
    if !(0..=1024).contains(&start) || !(1..=1024).contains(&end) || start >= end {
        return Err(format!(
            "{field}.start/end 必须满足 0 <= start < end <= 1024"
        ));
    }
    let target = int_of(value.get("target"), None).ok_or_else(|| format!("{field}.target 必填"))?;
    if !(0..=1024).contains(&target) || target + width > 1024 {
        return Err(format!("{field}.target 越界"));
    }

    let endian_text = match value.get("endian") {
        Some(value) if truthy(value) => as_text(value),
        _ => "little".to_string(),
    };
    let endian = match endian_text.trim().to_lowercase().as_str() {
        "little" => ChecksumEndian::Little,
        "big" => ChecksumEndian::Big,
        _ => return Err(format!("{field}.endian 只能是 little / big")),
    };
    let out_text = match value.get("out") {
        Some(value) if truthy(value) => as_text(value),
        _ => "raw".to_string(),
    };
    let out = match out_text.trim().to_lowercase().as_str() {
        "raw" => ChecksumOut::Raw,
        "not" => ChecksumOut::Not,
        "base_minus" => ChecksumOut::BaseMinus,
        _ => return Err(format!("{field}.out 只能是 raw / not / base_minus")),
    };
    let base = match int_of(value.get("base"), None) {
        None => None,
        Some(n) if (0..=0xFFFF).contains(&n) => Some(n as u16),
        Some(_) => return Err(format!("{field}.base 必须在 0..0xFFFF")),
    };
    if out == ChecksumOut::BaseMinus && base.is_none() {
        return Err(format!("{field}.out=base_minus 时必须给 base"));
    }
    let zero_target = value.get("zero_target").map(truthy).unwrap_or(false);

    Ok(Some(ChecksumSpec {
        algo,
        start: start as u16,
        end: end as u16,
        target: target as u16,
        endian,
        out,
        base,
        zero_target,
    }))
}

pub fn normalize(raw: &Value, source: &str, fallback_id: &str) -> Result<Profile, String> {
    if !raw.is_object() {
        return Err("profile 顶层必须是对象".to_string());
    }

    let id_text = match raw.get("id") {
        Some(value) if truthy(value) => as_text(value),
        _ => fallback_id.to_string(),
    };
    let id = id_text.trim().to_lowercase();
    if !valid_id(&id) {
        return Err("id 非法（只允许小写字母/数字/._-，最长 64）".to_string());
    }

    let version = int_of(raw.get("schema_version"), Some(1)).unwrap_or(1);
    if version != i64::from(SCHEMA_VERSION) {
        return Err(format!(
            "不支持的 schema_version={version}（当前 {SCHEMA_VERSION}）"
        ));
    }

    let mut known_ids = Vec::new();
    if let Some(items) = raw.get("known_ids").and_then(Value::as_array) {
        for item in items {
            if !item.is_object() {
                continue;
            }
            let vid = text_field(item, "vid").trim().to_uppercase();
            if vid.is_empty() {
                continue;
            }
            known_ids.push(KnownId {
                vid,
                pid: text_field(item, "pid").trim().to_uppercase(),
                wired_pid: text_field(item, "wired_pid").trim().to_uppercase(),
            });
        }
    }

    let match_raw = raw.get("match").filter(|value| value.is_object());
    let usage_page = int_of(sub(match_raw, "usage_page"), Some(0xFF02)).unwrap_or(0xFF02);
    if !(0..=0xFFFF).contains(&usage_page) {
        return Err("match.usage_page 必须在 0..0xFFFF".to_string());
    }
    let usage = match sub(match_raw, "usage") {
        None | Some(Value::Null) => None,
        Some(value) => match as_int(value) {
            Some(n) if (0..=0xFFFF).contains(&n) => Some(n as u16),
            _ => return Err("match.usage 非法".to_string()),
        },
    };
    let require_pid_hit = sub(match_raw, "require_pid_hit")
        .map(truthy)
        .unwrap_or(false);
    let interface = match sub(match_raw, "interface") {
        None | Some(Value::Null) => None,
        Some(value) => match as_int(value) {
            Some(n) if (0..=255).contains(&n) => Some(n as u8),
            _ => return Err("match.interface 必须在 0..255".to_string()),
        },
    };
    let collection = match sub(match_raw, "collection") {
        None | Some(Value::Null) => None,
        Some(value) => match as_int(value) {
            Some(n) if (0..=255).contains(&n) => Some(n as u8),
            _ => return Err("match.collection 必须在 0..255".to_string()),
        },
    };
    let feature_len = match sub(match_raw, "feature_len") {
        None | Some(Value::Null) => None,
        Some(value) => match as_int(value) {
            Some(n) if (1..=1024).contains(&n) => Some(n as u16),
            _ => return Err("match.feature_len 必须在 1..1024".to_string()),
        },
    };
    let ignore_usage_page = sub(match_raw, "ignore_usage_page")
        .map(truthy)
        .unwrap_or(false);

    let request_raw = raw.get("request").filter(|value| value.is_object());
    let transport_text = match sub(request_raw, "transport") {
        Some(value) if truthy(value) => as_text(value),
        _ => "write_read".to_string(),
    };
    let transport = match transport_text.trim().to_lowercase().as_str() {
        "write_read" => Transport::WriteRead,
        "feature_read" => Transport::FeatureRead,
        "feature_write_read" => Transport::FeatureWriteRead,
        "passive" => Transport::Passive,
        _ => {
            return Err(
                "request.transport 只能是 write_read / feature_read / feature_write_read / passive"
                    .to_string(),
            );
        }
    };
    // feature_read / passive 完全不看 payload_hex（V16 同样忽略，哪怕是非法写法）；
    // feature_write_read 的查询帧在 payload 里，必须解析。
    let payload = if matches!(
        transport,
        Transport::WriteRead | Transport::FeatureWriteRead
    ) {
        hex_bytes(sub(request_raw, "payload_hex"))?
    } else {
        Vec::new()
    };

    let out_len = match int_of(sub(request_raw, "out_len"), None) {
        None => None,
        Some(n) if (1..=1024).contains(&n) => Some(n as u16),
        Some(_) => return Err("request.out_len 必须在 1..1024".to_string()),
    };
    let in_len = match int_of(sub(request_raw, "in_len"), None) {
        None => None,
        Some(n) if (1..=1024).contains(&n) => Some(n as u16),
        Some(_) => return Err("request.in_len 必须在 1..1024".to_string()),
    };
    let report_id = match int_of(sub(request_raw, "report_id"), None) {
        None => None,
        Some(n) if (0..=255).contains(&n) => Some(n as u8),
        Some(_) => return Err("request.report_id 必须在 0..255".to_string()),
    };
    let write_mode = match sub(request_raw, "write_mode")
        .filter(|value| truthy(value))
        .map(as_text)
        .unwrap_or_else(|| "write_file".to_string())
        .trim()
        .to_lowercase()
        .as_str()
    {
        "write_file" => WriteMode::WriteFile,
        "set_output_report" => WriteMode::SetOutputReport,
        _ => return Err("request.write_mode 只能是 write_file / set_output_report".to_string()),
    };
    if write_mode == WriteMode::SetOutputReport && transport != Transport::WriteRead {
        return Err("request.write_mode=set_output_report 只允许用于 write_read".to_string());
    }
    let request_transform = parse_transform(sub(request_raw, "transform"), "request.transform")?;
    let request_checksum = checksum_spec(sub(request_raw, "checksum"), "request.checksum")?;

    let response_raw = raw.get("response").filter(|value| value.is_object());
    let min_length = int_of(sub(response_raw, "min_length"), Some(7)).unwrap_or(7);
    if !(1..=1024).contains(&min_length) {
        return Err("response.min_length 必须在 1..1024".to_string());
    }

    let exact_length = match int_of(sub(response_raw, "exact_length"), None) {
        None => None,
        Some(n) if (min_length..=1024).contains(&n) => Some(n as usize),
        Some(_) => return Err("response.exact_length 必须在 min_length..1024".to_string()),
    };
    let response_transform = parse_transform(sub(response_raw, "transform"), "response.transform")?;
    let response_checksum = checksum_spec(sub(response_raw, "checksum"), "response.checksum")?;
    let markers = match sub(response_raw, "markers") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                let pair = item
                    .as_array()
                    .filter(|pair| pair.len() == 2)
                    .ok_or_else(|| "response.markers 必须是 [offset, byte] 数组".to_string())?;
                let offset = offset_of(pair.first(), "response.markers.offset", None)?;
                let byte = as_int(&pair[1])
                    .filter(|n| (0..=255).contains(n))
                    .ok_or_else(|| "response.markers.byte 必须在 0..255".to_string())?;
                Ok((offset, byte as u8))
            })
            .collect::<Result<Vec<_>, String>>()?,
        Some(_) => return Err("response.markers 必须是数组".to_string()),
    };

    let status_raw = response_raw
        .and_then(|value| value.get("status"))
        .filter(|value| value.is_object());
    let status_offset = match sub(status_raw, "offset") {
        None | Some(Value::Null) => None,
        Some(value) => Some(offset_of(Some(value), "response.status.offset", None)?),
    };
    let ok_values = match sub(status_raw, "ok_values") {
        None => vec![0],
        Some(Value::Array(items)) if !items.is_empty() => items
            .iter()
            .map(|value| clamp_i32(as_int(value).unwrap_or(-9999)))
            .collect(),
        Some(_) => return Err("response.status.ok_values 必须是非空数组".to_string()),
    };

    let level_raw = response_raw
        .and_then(|value| value.get("level"))
        .filter(|value| value.is_object());
    let level_offset = offset_of(sub(level_raw, "offset"), "response.level.offset", Some(6))?;
    let encoding_text = match sub(level_raw, "encoding") {
        Some(value) if truthy(value) => as_text(value),
        _ => "u8".to_string(),
    };
    let encoding = match encoding_text.trim().to_lowercase().as_str() {
        "u8" => LevelEncoding::U8,
        "u16le" => LevelEncoding::U16Le,
        _ => return Err("response.level.encoding 只能是 u8 / u16le".to_string()),
    };
    let level_min = clamp_i32(int_of(sub(level_raw, "min"), Some(0)).unwrap_or(0));
    let level_max = clamp_i32(int_of(sub(level_raw, "max"), Some(100)).unwrap_or(100));
    if level_min > level_max {
        return Err("response.level.min 不能大于 max".to_string());
    }
    let scale = float_or(sub(level_raw, "scale"), 1.0, "response.level.scale")?;
    let add = float_or(sub(level_raw, "add"), 0.0, "response.level.add")?;
    let level_mask = int_of(sub(level_raw, "mask"), None).map(|value| value.clamp(0, 255) as u8);
    // `scale: 0` 已被 `or 1.0` 吞成 1.0；这条只在 `"scale": "0"` 这类字符串写法下触发（V16 同样如此）。
    if scale == 0.0 {
        return Err("response.level.scale 不能为 0".to_string());
    }

    let charging_raw = response_raw
        .and_then(|value| value.get("charging"))
        .filter(|value| value.is_object());
    let charging_offset = match sub(charging_raw, "offset") {
        None | Some(Value::Null) => None,
        Some(value) => Some(offset_of(Some(value), "response.charging.offset", None)?),
    };
    let when_charging = int_of(sub(charging_raw, "when_charging"), None).map(clamp_i32);
    let charging_nonzero = sub(charging_raw, "nonzero").map(truthy).unwrap_or(true);
    let charging_mask =
        int_of(sub(charging_raw, "mask"), None).map(|value| value.clamp(0, 255) as u8);

    let sampling_raw = raw.get("sampling").filter(|value| value.is_object());
    let attempts = int_of(sub(sampling_raw, "attempts"), Some(3)).unwrap_or(3);
    if !(1..=8).contains(&attempts) {
        return Err("sampling.attempts 必须在 1..8".to_string());
    }
    let stable_count = int_of(sub(sampling_raw, "stable_count"), Some(2)).unwrap_or(2);
    if !(1..=attempts).contains(&stable_count) {
        return Err("sampling.stable_count 必须在 1..attempts".to_string());
    }
    let request_timeout_ms =
        int_of(sub(sampling_raw, "request_timeout_ms"), Some(1200)).unwrap_or(1200);
    if !(100..=10_000).contains(&request_timeout_ms) {
        return Err("sampling.request_timeout_ms 必须在 100..10000".to_string());
    }
    let total_timeout_s = float_or(
        sub(sampling_raw, "total_timeout_s"),
        4.5,
        "sampling.total_timeout_s",
    )?;
    if !(1.0..=30.0).contains(&total_timeout_s) {
        return Err("sampling.total_timeout_s 必须在 1..30".to_string());
    }
    let pick_text = match sub(sampling_raw, "pick") {
        Some(value) if truthy(value) => as_text(value),
        _ => "mode_nonzero".to_string(),
    };
    let pick = match pick_text.trim().to_lowercase().as_str() {
        "mode_nonzero" => Pick::ModeNonzero,
        "last" => Pick::Last,
        "median" => Pick::Median,
        _ => return Err("sampling.pick 只能是 mode_nonzero / last / median".to_string()),
    };

    let passive = if transport == Transport::Passive {
        let passive_raw = raw.get("passive").filter(|value| value.is_object());
        let stale_after_s = float_or(
            sub(passive_raw, "stale_after_s"),
            15.0,
            "passive.stale_after_s",
        )?;
        if !(0.0..=86_400.0).contains(&stale_after_s) {
            return Err("passive.stale_after_s 必须在 0..86400".to_string());
        }
        let initial_wait_ms =
            int_of(sub(passive_raw, "initial_wait_ms"), Some(2500)).unwrap_or(2500);
        if !(0..=10_000).contains(&initial_wait_ms) {
            return Err("passive.initial_wait_ms 必须在 0..10000".to_string());
        }
        if markers.is_empty() && status_offset.is_none() {
            return Err("passive profile 必须至少有一条 markers 或 status.offset".to_string());
        }
        Some(PassiveRule {
            stale_after_s,
            initial_wait_ms: initial_wait_ms as u64,
        })
    } else {
        None
    };

    let label = {
        let text = text_field(raw, "label");
        if text.is_empty() { id.clone() } else { text }
    };
    let vendor = {
        let text = text_field(raw, "vendor");
        if text.is_empty() {
            "其他".to_string()
        } else {
            text
        }
    };
    let keywords = match raw.get("keywords") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" "),
        Some(value) if truthy(value) => as_text(value),
        _ => String::new(),
    };
    let model_code = match raw.get("model_code") {
        None | Some(Value::Null) => None,
        Some(value) => match as_int(value) {
            Some(n) if (0..=0xFFFF).contains(&n) => Some(n as u16),
            _ => return Err("model_code 必须在 0..0xFFFF".to_string()),
        },
    };

    Ok(Profile {
        id,
        label,
        vendor,
        keywords,
        model_code,
        schema_version: SCHEMA_VERSION,
        source: source.to_string(),
        enabled: raw.get("enabled").map(truthy).unwrap_or(true),
        notes: text_field(raw, "notes"),
        known_ids,
        passive,
        match_rule: MatchRule {
            usage_page: usage_page as u16,
            usage,
            interface,
            collection,
            feature_len,
            ignore_usage_page,
            require_pid_hit,
        },
        request: Request {
            transport,
            payload,
            out_len,
            in_len,
            report_id,
            read_matching: sub(request_raw, "read_matching")
                .map(truthy)
                .unwrap_or(false),
            write_mode,
            transform: request_transform,
            checksum: request_checksum,
        },
        response: Response {
            markers,
            exact_length,
            min_length: min_length as usize,
            transform: response_transform,
            checksum: response_checksum,
            status: StatusRule {
                offset: status_offset,
                ok_values,
            },
            level: LevelRule {
                offset: level_offset,
                encoding,
                min: level_min,
                max: level_max,
                scale,
                add,
                mask: level_mask,
            },
            charging: ChargingRule {
                offset: charging_offset,
                nonzero: charging_nonzero,
                when_charging,
                mask: charging_mask,
            },
        },
        sampling: Sampling {
            attempts: attempts as u8,
            stable_count: stable_count as u8,
            request_timeout_ms: request_timeout_ms as u64,
            total_timeout_s,
            pick,
        },
    })
}

// ---------- 加载 ----------

fn builtin_entries() -> Vec<(String, Value)> {
    let mut entries = Vec::new();
    for text in [BUILTIN_JSON, COMMUNITY_JSON, COMMUNITY2_JSON] {
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) {
            entries.extend(map);
        }
    }
    entries
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// 目录下的 `*.json`（Windows 文件名大小写不敏感，与 Python `glob` 一致）。
fn json_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        })
        .collect()
}

fn read_profile(path: &Path, source: &str) -> Result<(Profile, Value), String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let raw: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let profile = normalize(&raw, source, &file_stem(path))?;
    Ok((profile, raw))
}

/// 加载协议表：内置 → `dirs` 反转（父目录先、本级后覆盖）。
///
/// 返回 `(id -> profile, 错误列表)`。调用方想要 `profile-errors.log` 时，
/// 自己把错误列表交给 [`log_errors`]（V16 在 `load` 里直接写日志，这里拆开，
/// 因为路径由调用方决定）。
pub fn load(dirs: &[PathBuf]) -> (HashMap<String, Profile>, Vec<ProfileError>) {
    let mut table: HashMap<String, Profile> = HashMap::new();
    let mut errors: Vec<ProfileError> = Vec::new();

    for (id, raw) in builtin_entries() {
        match normalize(&raw, "builtin", &id) {
            Ok(profile) => {
                table.insert(profile.id.clone(), profile);
            }
            Err(error) => errors.push(ProfileError::new(format!("builtin:{id}"), error)),
        }
    }

    for dir in dirs.iter().rev() {
        let mut files = json_files(dir);
        files.sort();
        for path in files {
            let source = path.to_string_lossy().to_string();
            match read_profile(&path, &source) {
                Ok((mut profile, raw)) => {
                    if profile.enabled {
                        // 同 id 整体覆盖内置/前一个文件，协议字段不做合并；
                        // 但 vendor/keywords 是纯展示字段，外置 JSON 没写时
                        // 沿用被覆盖条目，免得条目掉回「其他」组、搜索失联。
                        if let Some(old) = table.get(&profile.id) {
                            if raw.get("vendor").is_none() {
                                profile.vendor = old.vendor.clone();
                            }
                            if raw.get("keywords").is_none() {
                                profile.keywords = old.keywords.clone();
                            }
                        }
                        table.insert(profile.id.clone(), profile);
                    } else {
                        // `enabled: false` 连内置一起移除。
                        table.remove(&profile.id);
                    }
                }
                Err(error) => errors.push(ProfileError::new(source, error)),
            }
        }
    }

    for error in &errors {
        eprintln!("[profile] 跳过 {}: {}", error.file, error.error);
    }
    (table, errors)
}

// ---------- 选协议 ----------

/// 归一化后的候选 PID 列表（去空、去重）。
fn candidate_pids(pid: &str, wired_pid: &str) -> Vec<String> {
    let mut pids: Vec<String> = Vec::new();
    for candidate in [pid.trim().to_uppercase(), wired_pid.trim().to_uppercase()] {
        if !candidate.is_empty() && !pids.contains(&candidate) {
            pids.push(candidate);
        }
    }
    pids
}

/// 同 VID/PID 精确命中的所有 profile（不含只写 VID 的通配条目），按 id 字典序。
///
/// 多个品牌共用一颗公版接收器时（如 `3554:FA09`），调用方可再按会话 ACK
/// 型号码从这批候选里选真正归属的协议。
pub fn exact_matches<'a>(
    vid: &str,
    pid: &str,
    wired_pid: &str,
    table: &'a HashMap<String, Profile>,
) -> Vec<&'a Profile> {
    let vid = vid.trim().to_uppercase();
    if vid.is_empty() {
        return Vec::new();
    }
    let pids = candidate_pids(pid, wired_pid);
    if pids.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<&Profile> = table
        .values()
        .filter(|profile| {
            profile
                .known_ids
                .iter()
                .any(|known| known.vid == vid && !known.pid.is_empty() && pids.contains(&known.pid))
        })
        .collect();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

/// 按 vid/pid/wired_pid 反查 profile（`known_ids` 匹配，不看 `match`）。
///
/// V16 依赖 dict 插入顺序取第一个命中；`HashMap` 无序，这里按 id 字典序取最小者，
/// 保证结果确定（多个 profile 同时命中时可能与 V16 选到不同的那个，见差异说明）。
pub fn for_ids<'a>(
    vid: &str,
    pid: &str,
    wired_pid: &str,
    table: &'a HashMap<String, Profile>,
) -> Option<&'a Profile> {
    let vid = vid.trim().to_uppercase();
    if vid.is_empty() {
        return None;
    }
    let pids = candidate_pids(pid, wired_pid);
    // 精确 PID 命中优先于 vid 通配：0B05/36F9 全系通配条目不能抢走
    // Omni、PELTA、X7 等带明确 PID 的协议（同表字典序裁决时会踩到）。
    let exact = exact_matches(&vid, pid, wired_pid, table);
    if let Some(profile) = exact.first() {
        return Some(profile);
    }
    table
        .iter()
        .filter(|(_, profile)| {
            profile.known_ids.iter().any(|known| {
                known.vid == vid
                    && (pids.is_empty() || known.pid.is_empty() || pids.contains(&known.pid))
            })
        })
        .min_by(|a, b| a.0.cmp(b.0))
        .map(|(_, profile)| profile)
}

/// 解析顺序：显式 `dev.profile` → `known_ids` 推断 → `key == "mouse"` 默认 profile。
///
/// 显式 profile 不存在时返回 `None`（调用方按 error 处理，不静默回退）。
pub fn for_device<'a>(
    dev: &DeviceSpec,
    table: &'a HashMap<String, Profile>,
) -> Option<&'a Profile> {
    let explicit = dev.profile.trim().to_lowercase();
    if !explicit.is_empty() {
        return table.get(&explicit);
    }
    if let Some(profile) = for_ids(&dev.vid, &dev.pid, &dev.wired_pid, table) {
        return Some(profile);
    }
    if dev.key == "mouse" {
        return table.get(DEFAULT_PROFILE);
    }
    None
}

// ---------- 错误日志 ----------

/// 追加错误到 `dir/profile-errors.log`，只保留最后 200 行；任何失败都静默。
pub fn log_errors(dir: &Path, errors: &[ProfileError]) {
    if errors.is_empty() {
        return;
    }
    let _ = write_errors(dir, errors);
}

fn write_errors(dir: &Path, errors: &[ProfileError]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join("profile-errors.log");
    // 读不到/非法 UTF-8 都当作空历史（V16 用 errors="replace"，这里退化为丢弃旧行）。
    let existing = std::fs::read(&path)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    let mut lines: Vec<String> = existing.lines().map(str::to_string).collect();
    if lines.len() > 160 {
        lines.drain(..lines.len() - 160);
    }
    let stamp = timestamp();
    for error in errors {
        lines.push(format!("[{stamp}] {}: {}", error.file, error.error));
    }
    if lines.len() > 200 {
        lines.drain(..lines.len() - 200);
    }
    let mut text = lines.join("\n");
    text.push('\n');
    std::fs::write(&path, text)
}

/// `[YYYY-MM-DD HH:MM:SS]`（本地时间，对应 V16 的 `time.strftime`）。
fn timestamp() -> String {
    let (year, month, day, hour, minute, second) = local_parts();
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

/// Windows 本地时间。`GetLocalTime` 属于 `Win32_System_SystemInformation`，
/// 当前 Cargo.toml 没开这个 feature（也不允许改），所以这里直接声明。
#[cfg(windows)]
fn local_parts() -> (i32, u32, u32, u32, u32, u32) {
    #[repr(C)]
    struct SystemTime {
        year: u16,
        month: u16,
        _day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        _milliseconds: u16,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocalTime(lp_system_time: *mut SystemTime);
    }

    let mut st = SystemTime {
        year: 0,
        month: 0,
        _day_of_week: 0,
        day: 0,
        hour: 0,
        minute: 0,
        second: 0,
        _milliseconds: 0,
    };
    // SAFETY: 指针指向本函数栈上的 SYSTEMTIME（8 × u16，与 Win32 布局一致），
    // GetLocalTime 只写这 16 字节，不会越界。
    unsafe { GetLocalTime(&mut st) };
    (
        i32::from(st.year),
        u32::from(st.month),
        u32::from(st.day),
        u32::from(st.hour),
        u32::from(st.minute),
        u32::from(st.second),
    )
}

/// 非 Windows 目标没有 `GetLocalTime`，退化为 UTC（本项目只在 Windows 运行）。
#[cfg(not(windows))]
fn local_parts() -> (i32, u32, u32, u32, u32, u32) {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    (
        year as i32,
        month as u32,
        day as u32,
        (rest / 3600) as u32,
        (rest % 3600 / 60) as u32,
        (rest % 60) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_builtin_and_community_profiles() {
        let (table, errors) = load(&[]);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(table.len(), 98, "57 builtin + 17 community + 24 community2");
        assert!(table.contains_key("mchose-kb-ff04"));
        assert!(table.contains_key("nzxt-lift-elite"));
        assert_eq!(
            table["raptoo-vt-passive"].request.transport,
            Transport::Passive
        );
        assert!(table["raptoo-vt-passive"].passive.is_some());
        assert_eq!(
            for_ids("373B", "1031", "", &table).map(|profile| profile.id.as_str()),
            Some("atk-f1-family")
        );
        assert_eq!(
            for_ids("1E71", "2101", "", &table).map(|profile| profile.id.as_str()),
            Some("nzxt-lift-elite")
        );
    }

    #[test]
    fn parses_checksum_and_interface_fields() {
        let raw = serde_json::json!({
            "id": "test-checksum",
            "known_ids": [{"vid": "1234", "pid": "5678"}],
            "match": {"usage_page": "0xFF02", "interface": 2, "collection": 3},
            "request": {
                "transport": "write_read",
                "payload_hex": "00 01 02 03 00",
                "out_len": 5,
                "checksum": {
                    "algo": "sum8", "start": 0, "end": 5, "target": 4,
                    "out": "base_minus", "base": 85, "zero_target": true
                }
            },
            "response": {
                "min_length": 5,
                "checksum": {
                    "algo": "crc16_ccitt_false", "start": 0, "end": 3, "target": 3
                }
            }
        });
        let profile = normalize(&raw, "test", "test-checksum").unwrap();
        assert_eq!(profile.match_rule.interface, Some(2));
        assert_eq!(profile.match_rule.collection, Some(3));
        let checksum = profile.request.checksum.unwrap();
        assert_eq!(checksum.out, ChecksumOut::BaseMinus);
        assert!(profile.response.checksum.is_some());
    }

    #[test]
    fn rejects_bad_checksum_range() {
        let raw = serde_json::json!({
            "id": "bad-checksum",
            "request": {
                "transport": "write_read",
                "payload_hex": "00",
                "checksum": {"algo": "sum8", "start": 4, "end": 2, "target": 0}
            }
        });
        assert!(normalize(&raw, "test", "bad-checksum").is_err());
    }
}
