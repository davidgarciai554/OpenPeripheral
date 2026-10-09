// Generated from libratbag (https://github.com/libratbag/libratbag, commit
// 8235b5bb, MIT): `data/devices/asus-*.device`. Copyright (C) 2021
// Kyoken and the libratbag contributors; see `protocol/asus.rs`. Models
// behind the Omni receiver or with separate X/Y DPI are left out.

use super::Model;

pub static MODELS: &[Model] = &[
    // asus-rog-buzzard.device
    Model {
        name: "ASUS ROG GX860 Buzzard Mouse",
        pid: 0x1816,
        presets: 2,
        dpi: (100, 8200, 100),
        double_dpi: false,
        settings_v2: false,
    },
    // asus-rog-chakram-core.device
    Model {
        name: "ASUS ROG Chakram Core",
        pid: 0x1958,
        presets: 4,
        dpi: (100, 16000, 100),
        double_dpi: true,
        settings_v2: false,
    },
    // asus-rog-chakram.device
    Model {
        name: "ASUS ROG Chakram",
        pid: 0x18e3,
        presets: 4,
        dpi: (100, 16000, 100),
        double_dpi: true,
        settings_v2: false,
    },
    // asus-rog-chakram.device
    Model {
        name: "ASUS ROG Chakram",
        pid: 0x18e5,
        presets: 4,
        dpi: (100, 16000, 100),
        double_dpi: true,
        settings_v2: false,
    },
    // asus-rog-gladius2-origin-pink.device
    Model {
        name: "ASUS ROG Gladius II Origin PNK LTD",
        pid: 0x18cd,
        presets: 2,
        dpi: (100, 12000, 100),
        double_dpi: true,
        settings_v2: false,
    },
    // asus-rog-gladius2-origin.device
    Model {
        name: "ASUS ROG Gladius II Origin",
        pid: 0x1877,
        presets: 2,
        dpi: (100, 12000, 100),
        double_dpi: true,
        settings_v2: false,
    },
    // asus-rog-gladius2.device
    Model {
        name: "ASUS ROG Gladius II",
        pid: 0x1845,
        presets: 2,
        dpi: (100, 12000, 100),
        double_dpi: false,
        settings_v2: false,
    },
    // asus-rog-harpe-wireless.device
    Model {
        name: "ASUS ROG Harpe Wireless",
        pid: 0x1a92,
        presets: 4,
        dpi: (100, 36000, 50),
        double_dpi: false,
        settings_v2: false,
    },
    // asus-rog-keris-wireless-aimpoint.device
    Model {
        name: "ASUS ROG Keris Wireless AimPoint",
        pid: 0x1a66,
        presets: 4,
        dpi: (100, 36000, 50),
        double_dpi: false,
        settings_v2: false,
    },
    // asus-rog-keris-wireless-aimpoint.device
    Model {
        name: "ASUS ROG Keris Wireless AimPoint",
        pid: 0x1a68,
        presets: 4,
        dpi: (100, 36000, 50),
        double_dpi: false,
        settings_v2: false,
    },
    // asus-rog-keris-wireless.device
    Model {
        name: "ASUS ROG Keris Wireless",
        pid: 0x195e,
        presets: 4,
        dpi: (100, 16000, 100),
        double_dpi: true,
        settings_v2: false,
    },
    // asus-rog-keris-wireless.device
    Model {
        name: "ASUS ROG Keris Wireless",
        pid: 0x1960,
        presets: 4,
        dpi: (100, 16000, 100),
        double_dpi: true,
        settings_v2: false,
    },
    // asus-rog-keris.device
    Model {
        name: "ASUS ROG Keris",
        pid: 0x195c,
        presets: 4,
        dpi: (100, 16000, 100),
        double_dpi: true,
        settings_v2: false,
    },
    // asus-rog-pugio.device
    Model {
        name: "ASUS ROG Pugio",
        pid: 0x1846,
        presets: 2,
        dpi: (100, 7200, 100),
        double_dpi: false,
        settings_v2: false,
    },
    // asus-rog-spatha-x.device
    Model {
        name: "ASUS ROG Spatha X",
        pid: 0x1979,
        presets: 4,
        dpi: (100, 19000, 50),
        double_dpi: false,
        settings_v2: false,
    },
    // asus-rog-spatha-x.device
    Model {
        name: "ASUS ROG Spatha X",
        pid: 0x1977,
        presets: 4,
        dpi: (100, 19000, 50),
        double_dpi: false,
        settings_v2: false,
    },
    // asus-rog-strix-carry.device
    Model {
        name: "ASUS ROG Strix Carry",
        pid: 0x18b4,
        presets: 2,
        dpi: (100, 7200, 100),
        double_dpi: false,
        settings_v2: false,
    },
    // asus-rog-strix-impact.device
    Model {
        name: "ASUS ROG Strix Impact",
        pid: 0x1847,
        presets: 2,
        dpi: (100, 5000, 100),
        double_dpi: false,
        settings_v2: false,
    },
    // asus-rog-strix-impact2-wireless.device
    Model {
        name: "ASUS ROG Strix Impact II Wireless",
        pid: 0x1947,
        presets: 4,
        dpi: (100, 16000, 100),
        double_dpi: false,
        settings_v2: false,
    },
    // asus-rog-strix-impact2-wireless.device
    Model {
        name: "ASUS ROG Strix Impact II Wireless",
        pid: 0x1949,
        presets: 4,
        dpi: (100, 16000, 100),
        double_dpi: false,
        settings_v2: false,
    },
    // asus-rog-strix-impact2.device
    Model {
        name: "ASUS ROG Strix Impact II",
        pid: 0x18e1,
        presets: 4,
        dpi: (200, 6200, 100),
        double_dpi: false,
        settings_v2: false,
    },
    // asus-rog-strix-impact3.device
    Model {
        name: "ASUS ROG Strix Impact III",
        pid: 0x1a88,
        presets: 4,
        dpi: (100, 12000, 50),
        double_dpi: false,
        settings_v2: true,
    },
    // asus-tuf-gaming-m3-gen-ii.device
    Model {
        name: "ASUSTeK TUF GAMING M3 GEN II",
        pid: 0x1a9b,
        presets: 4,
        dpi: (100, 8000, 100),
        double_dpi: false,
        settings_v2: false,
    },
    // asus-tuf-gaming-m4-air.device
    Model {
        name: "ASUS TUF GAMING M4 AIR",
        pid: 0x1a03,
        presets: 4,
        dpi: (100, 16000, 100),
        double_dpi: false,
        settings_v2: false,
    },
];
