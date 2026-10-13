// Model names transcribed from OpenMouse (https://github.com/OpenMouse-Project/openmouse,
// commit 59b5ac0b, AGPL-3.0-or-later): `src/supported-devices-data.ts`
// (brand, model, product ids) and the vendor ids of `VENDOR_ID` in
// `drivers/vendors.js` of @openmouse/protocol 0.40.1. Each product id was
// checked to appear in that package. Brands with several possible vendor
// ids (Glorious, HyperX, Lamzu, Attack Shark, Ninjutso, MCHOSE, Redragon,
// WALLHACK, K-snake) are left out, so a name is never guessed.
//
// Some of these ids come from OpenMouse modules derived from vendor
// software (AJAZZ, ATK, Dareu, Finalmouse, Incott, Keychron, VAXEE):
// provenance not verified, kept by the decision of 2026-10-13 (see
// CONTRIBUTING.md). Each brand is its own block so it can be removed.
//
// Names only: these mice are listed, never opened nor written.

/// A mouse known by name only.
#[derive(Debug, PartialEq, Eq)]
pub struct ListedModel {
    pub vid: u16,
    pub pid: u16,
    pub vendor: &'static str,
    pub model: &'static str,
}

/// Simplification: a hand-kept table (under 100 entries). Move to a
/// generator if it grows past that or OpenMouse changes it often.
#[rustfmt::skip]
pub static LISTED: [ListedModel; 99] = [
    // AJAZZ
    ListedModel { vid: 0x1a2c, pid: 0x2157, vendor: "AJAZZ", model: "NJ07" },
    ListedModel { vid: 0x1a2c, pid: 0x2158, vendor: "AJAZZ", model: "NJ08" },
    ListedModel { vid: 0x1a2c, pid: 0x2167, vendor: "AJAZZ", model: "NJ07 MC" },
    ListedModel { vid: 0x1a2c, pid: 0x2168, vendor: "AJAZZ", model: "NJ08 MC" },
    ListedModel { vid: 0x1a2c, pid: 0x2177, vendor: "AJAZZ", model: "NJ07 MC PRO" },
    ListedModel { vid: 0x1a2c, pid: 0x2178, vendor: "AJAZZ", model: "NJ08 MC PRO" },
    // ATK
    ListedModel { vid: 0x373b, pid: 0x1154, vendor: "ATK", model: "ZERO" },
    ListedModel { vid: 0x373b, pid: 0x1155, vendor: "ATK", model: "ZERO" },
    // Corsair
    ListedModel { vid: 0x1b1c, pid: 0x1b4c, vendor: "Corsair", model: "Ironclaw RGB Wireless" },
    ListedModel { vid: 0x1b1c, pid: 0x1b5c, vendor: "Corsair", model: "NIGHTSWORD RGB" },
    ListedModel { vid: 0x1b1c, pid: 0x1b66, vendor: "Corsair", model: "Ironclaw RGB Wireless" },
    ListedModel { vid: 0x1b1c, pid: 0x1bdc, vendor: "Corsair", model: "Ironclaw RGB Wireless" },
    // Dareu
    ListedModel { vid: 0x260d, pid: 0x1114, vendor: "Dareu", model: "A950 PRO Mg" },
    ListedModel { vid: 0x260d, pid: 0x1117, vendor: "Dareu", model: "A950 PRO Mg" },
    // Endgame Gear
    ListedModel { vid: 0x3367, pid: 0x1903, vendor: "Endgame Gear", model: "XM1r" },
    ListedModel { vid: 0x3367, pid: 0x1905, vendor: "Endgame Gear", model: "OP1 RGB" },
    ListedModel { vid: 0x3367, pid: 0x1906, vendor: "Endgame Gear", model: "OP1" },
    ListedModel { vid: 0x3367, pid: 0x1960, vendor: "Endgame Gear", model: "XM2we" },
    ListedModel { vid: 0x3367, pid: 0x1961, vendor: "Endgame Gear", model: "OP1we" },
    ListedModel { vid: 0x3367, pid: 0x1962, vendor: "Endgame Gear", model: "OP1we" },
    ListedModel { vid: 0x3367, pid: 0x1964, vendor: "Endgame Gear", model: "OP1 8K" },
    ListedModel { vid: 0x3367, pid: 0x1966, vendor: "Endgame Gear", model: "XM2 8K" },
    ListedModel { vid: 0x3367, pid: 0x1968, vendor: "Endgame Gear", model: "XM2w 4K" },
    ListedModel { vid: 0x3367, pid: 0x1970, vendor: "Endgame Gear", model: "OP1w 4K" },
    ListedModel { vid: 0x3367, pid: 0x1972, vendor: "Endgame Gear", model: "OP1w 4K" },
    ListedModel { vid: 0x3367, pid: 0x1976, vendor: "Endgame Gear", model: "OP1 8K Purple Frost" },
    ListedModel { vid: 0x3367, pid: 0x1978, vendor: "Endgame Gear", model: "OP1 8K v2" },
    ListedModel { vid: 0x3367, pid: 0x1980, vendor: "Endgame Gear", model: "XM2 8K v2" },
    ListedModel { vid: 0x3367, pid: 0x1982, vendor: "Endgame Gear", model: "XM2w 4K v2" },
    ListedModel { vid: 0x3367, pid: 0x1984, vendor: "Endgame Gear", model: "OP1w 4K v2" },
    // Fater
    ListedModel { vid: 0x04d9, pid: 0xa09f, vendor: "Fater", model: "MCR-9000B" },
    // Finalmouse
    ListedModel { vid: 0x361d, pid: 0x0100, vendor: "Finalmouse", model: "Starlight-12 / ULX" },
    // Incott
    ListedModel { vid: 0x093a, pid: 0x522c, vendor: "Incott", model: "G23 V2 (SE / Pro)" },
    ListedModel { vid: 0x093a, pid: 0x622c, vendor: "Incott", model: "G23 V2 (SE / Pro)" },
    // Keychron
    ListedModel { vid: 0x3434, pid: 0x0440, vendor: "Keychron", model: "Nape Pro" },
    ListedModel { vid: 0x3434, pid: 0xd029, vendor: "Keychron", model: "M6" },
    ListedModel { vid: 0x3434, pid: 0xd060, vendor: "Keychron", model: "M6" },
    // LunaFury
    ListedModel { vid: 0x373e, pid: 0x0032, vendor: "LunaFury", model: "LUNA33" },
    ListedModel { vid: 0x373e, pid: 0x0033, vendor: "LunaFury", model: "LUNA33" },
    ListedModel { vid: 0x373e, pid: 0x0054, vendor: "LunaFury", model: "TYPE33" },
    ListedModel { vid: 0x373e, pid: 0x0084, vendor: "LunaFury", model: "TYPE33" },
    // Microsoft
    ListedModel { vid: 0x045e, pid: 0x0823, vendor: "Microsoft", model: "Classic Intellimouse" },
    ListedModel { vid: 0x045e, pid: 0x082a, vendor: "Microsoft", model: "Pro Intellimouse" },
    // Orbital
    ListedModel { vid: 0x1915, pid: 0x080c, vendor: "Orbital", model: "Ghost / Pathfinder V2" },
    // Pulsar
    ListedModel { vid: 0x3710, pid: 0x3404, vendor: "Pulsar", model: "X2A V3" },
    ListedModel { vid: 0x3710, pid: 0x3409, vendor: "Pulsar", model: "X3 Medium" },
    ListedModel { vid: 0x3710, pid: 0x3410, vendor: "Pulsar", model: "X3 Medium" },
    ListedModel { vid: 0x3710, pid: 0x5402, vendor: "Pulsar", model: "X3 Medium" },
    ListedModel { vid: 0x3710, pid: 0x5403, vendor: "Pulsar", model: "X3 Medium" },
    // Rapoo
    ListedModel { vid: 0x24ae, pid: 0x1205, vendor: "Rapoo", model: "VT9 Pro" },
    ListedModel { vid: 0x24ae, pid: 0x4405, vendor: "Rapoo", model: "VT9 Pro" },
    // RAWM
    ListedModel { vid: 0x1915, pid: 0x2328, vendor: "RAWM", model: "Blade 1" },
    ListedModel { vid: 0x1915, pid: 0x2329, vendor: "RAWM", model: "ML01" },
    ListedModel { vid: 0x1915, pid: 0x232c, vendor: "RAWM", model: "MH01" },
    ListedModel { vid: 0x1915, pid: 0x232d, vendor: "RAWM", model: "SL01" },
    ListedModel { vid: 0x1915, pid: 0x232e, vendor: "RAWM", model: "SH01" },
    ListedModel { vid: 0x1915, pid: 0x232f, vendor: "RAWM", model: "SH01" },
    ListedModel { vid: 0x1915, pid: 0x2330, vendor: "RAWM", model: "SH01 Pro" },
    ListedModel { vid: 0x1915, pid: 0x2331, vendor: "RAWM", model: "MH01 Pro" },
    ListedModel { vid: 0x1915, pid: 0x2332, vendor: "RAWM", model: "SH01 Pro" },
    ListedModel { vid: 0x1915, pid: 0x2333, vendor: "RAWM", model: "ES21" },
    ListedModel { vid: 0x1915, pid: 0x2334, vendor: "RAWM", model: "ES21 Pro" },
    ListedModel { vid: 0x1915, pid: 0x2337, vendor: "RAWM", model: "ES21M" },
    ListedModel { vid: 0x1915, pid: 0x2338, vendor: "RAWM", model: "ER21 Pro" },
    ListedModel { vid: 0x1915, pid: 0x2339, vendor: "RAWM", model: "ER21M" },
    // Ryunix
    ListedModel { vid: 0x04f3, pid: 0x026e, vendor: "Ryunix", model: "KYU Pro MX1" },
    ListedModel { vid: 0x04f3, pid: 0x026f, vendor: "Ryunix", model: "KYU Pro MX1" },
    // Teevolution
    ListedModel { vid: 0x3554, pid: 0xf520, vendor: "Teevolution", model: "Terra Pro" },
    ListedModel { vid: 0x3554, pid: 0xf522, vendor: "Teevolution", model: "Terra Pro" },
    ListedModel { vid: 0x3554, pid: 0xf523, vendor: "Teevolution", model: "Terra Pro" },
    ListedModel { vid: 0x3554, pid: 0xf5bb, vendor: "Teevolution", model: "Terra Pro" },
    // VAXEE
    ListedModel { vid: 0x3057, pid: 0x1003, vendor: "VAXEE", model: "XE Wireless" },
    ListedModel { vid: 0x3057, pid: 0x1004, vendor: "VAXEE", model: "NP-01S Wireless" },
    ListedModel { vid: 0x3057, pid: 0x1005, vendor: "VAXEE", model: "AX Wireless" },
    ListedModel { vid: 0x3057, pid: 0x1006, vendor: "VAXEE", model: "NP-01 Wireless" },
    ListedModel { vid: 0x3057, pid: 0x1007, vendor: "VAXEE", model: "XE-S Wireless" },
    ListedModel { vid: 0x3057, pid: 0x1008, vendor: "VAXEE", model: "XE-S-L Wireless" },
    ListedModel { vid: 0x3057, pid: 0x1009, vendor: "VAXEE", model: "VAXEE x Ninjutso Sora" },
    ListedModel { vid: 0x3057, pid: 0x1010, vendor: "VAXEE", model: "E1 Wireless" },
    ListedModel { vid: 0x3057, pid: 0x1011, vendor: "VAXEE", model: "NP-01S V2 Wireless" },
    ListedModel { vid: 0x3057, pid: 0x1012, vendor: "VAXEE", model: "XE V2 Wireless" },
    ListedModel { vid: 0x3057, pid: 0x1013, vendor: "VAXEE", model: "NP-01S Ergo Wireless" },
    ListedModel { vid: 0x3057, pid: 0x1014, vendor: "VAXEE", model: "NP-01S V3 Wireless" },
    ListedModel { vid: 0x3057, pid: 0x1015, vendor: "VAXEE", model: "NP-01 Ergo Wireless" },
    // VGN
    ListedModel { vid: 0x3554, pid: 0xfb56, vendor: "VGN", model: "Dragonfly F2 Master+" },
    ListedModel { vid: 0x3554, pid: 0xfb57, vendor: "VGN", model: "Dragonfly F2 Master+" },
    // WLMouse
    ListedModel { vid: 0x36a7, pid: 0xa870, vendor: "WLMouse", model: "Beast X Pro" },
    ListedModel { vid: 0x36a7, pid: 0xa871, vendor: "WLMouse", model: "Beast X Pro" },
    ListedModel { vid: 0x36a7, pid: 0xa878, vendor: "WLMouse", model: "Sword X" },
    ListedModel { vid: 0x36a7, pid: 0xa879, vendor: "WLMouse", model: "Sword X" },
    ListedModel { vid: 0x36a7, pid: 0xa880, vendor: "WLMouse", model: "Beast Max" },
    ListedModel { vid: 0x36a7, pid: 0xa881, vendor: "WLMouse", model: "Beast Max" },
    ListedModel { vid: 0x36a7, pid: 0xa883, vendor: "WLMouse", model: "Beast X" },
    ListedModel { vid: 0x36a7, pid: 0xa884, vendor: "WLMouse", model: "Beast X" },
    ListedModel { vid: 0x36a7, pid: 0xa885, vendor: "WLMouse", model: "Beast Mini" },
    ListedModel { vid: 0x36a7, pid: 0xa886, vendor: "WLMouse", model: "Beast Mini" },
    ListedModel { vid: 0x36a7, pid: 0xa887, vendor: "WLMouse", model: "Beast X 4K" },
    // Zaunkoenig
    ListedModel { vid: 0x0483, pid: 0xa3cf, vendor: "Zaunkoenig", model: "M2K" },
    ListedModel { vid: 0x0483, pid: 0xa462, vendor: "Zaunkoenig", model: "M3K" },
];

pub fn model(vid: u16, pid: u16) -> Option<&'static ListedModel> {
    LISTED.iter().find(|m| (m.vid, m.pid) == (vid, pid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_names_present() {
        for (i, a) in LISTED.iter().enumerate() {
            assert!(!a.vendor.is_empty() && !a.model.is_empty());
            assert!(
                LISTED[i + 1..]
                    .iter()
                    .all(|b| (a.vid, a.pid) != (b.vid, b.pid)),
                "{:04x}:{:04x} twice",
                a.vid,
                a.pid
            );
        }
    }

    #[test]
    fn never_shadows_a_driver() {
        // Brands with a driver are found by `lookup`, not by name.
        for m in &LISTED {
            assert!(![0x046d, 0x1532, 0x1038, 0x0b05].contains(&m.vid), "{m:?}");
            assert!(
                super::super::SUPPORTED
                    .iter()
                    .all(|s| (s.vid, s.pid) != (m.vid, m.pid))
            );
        }
    }

    #[test]
    fn finds_by_vid_and_pid() {
        let m = model(0x3710, 0x3404).unwrap();
        assert_eq!((m.vendor, m.model), ("Pulsar", "X2A V3"));
        assert!(model(0x3710, 0x0001).is_none());
    }
}
