//! SteelSeries mice: the DPI output report (pure, no I/O). Data of each
//! model from rivalcfg (WTFPL, `steelseries_data.rs`). The device never
//! reports its DPI and this app never sends the "save" command, so a DPI
//! set here is lost when the mouse is unplugged (volatile).

#[path = "steelseries_data.rs"]
mod data;
pub use data::MODELS;

/// How a DPI becomes the sensor's code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// Table of (DPI, code) in steps of 100.
    Choice(&'static [(u16, u8)]),
    /// `out_min + (dpi - min) / step * out_step`.
    Linear { out_min: u16, out_step: u16 },
}

#[derive(Debug, PartialEq, Eq)]
pub struct Model {
    pub name: &'static str,
    pub pid: u16,
    /// USB interface that takes the report.
    pub interface: i32,
    pub command: &'static [u8],
    /// Accepted DPI: (min, max, step).
    pub dpi: (u16, u16, u16),
    pub encoding: Encoding,
    /// Bytes of each code, little-endian.
    pub dpi_bytes: usize,
    /// Index of the first preset in the report (0 or 1).
    pub first_preset: u8,
}

pub fn model(pid: u16) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.pid == pid)
}

impl Model {
    pub fn range(&self) -> super::DpiRange {
        let (min, max, step) = self.dpi;
        super::DpiRange::Range { min, max, step }
    }

    /// The output report (report id 0 first) that makes `dpi` the only
    /// preset, selected. `None` for a DPI the model does not accept.
    pub fn dpi_report(&self, dpi: u16) -> Option<Vec<u8>> {
        if !self.range().contains(dpi) {
            return None;
        }
        let (min, _, step) = self.dpi;
        let code = match self.encoding {
            Encoding::Choice(table) => table.iter().find(|&&(d, _)| d == dpi)?.1 as u32,
            Encoding::Linear { out_min, out_step } => {
                out_min as u32 + (dpi - min) as u32 / step as u32 * out_step as u32
            }
        };
        let mut r = vec![0x00];
        r.extend_from_slice(self.command);
        // One preset, the selected one.
        r.extend_from_slice(&[1, self.first_preset]);
        r.extend_from_slice(&code.to_le_bytes()[..self.dpi_bytes]);
        Some(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_match_rivalcfg_encodings() {
        // Rival 3: command 0B 00, TrueMove Core (800 -> 0x12? from table).
        let rival3 = model(0x1824).unwrap();
        let code = data::TRUEMOVE_CORE.iter().find(|e| e.0 == 800).unwrap().1;
        assert_eq!(
            rival3.dpi_report(800).unwrap(),
            vec![0, 0x0b, 0x00, 1, 1, code]
        );
        assert_eq!(rival3.dpi_report(150), None, "not a step");
        assert_eq!(rival3.dpi_report(8600), None, "above the sensor");
        // Prime: 2-byte linear code, 50 -> 1, 18000 -> 360.
        let prime = model(0x182e).unwrap();
        assert_eq!(prime.dpi_report(50).unwrap(), vec![0, 0x61, 1, 0, 1, 0]);
        assert_eq!(
            prime.dpi_report(18000).unwrap(),
            vec![0, 0x61, 1, 0, 0x68, 0x01]
        );
        // Rival 3 Wireless: table code on 2 bytes.
        let r3w = model(0x1830).unwrap();
        assert_eq!(
            r3w.dpi_report(100).unwrap(),
            vec![0, 0x20, 1, 1, 0x00, 0x00]
        );
        // Every model: every accepted DPI encodes, the tables cover the range.
        for m in MODELS {
            let (min, max, step) = m.dpi;
            for dpi in (min..=max).step_by(step as usize) {
                assert!(m.dpi_report(dpi).is_some(), "{} {dpi}", m.name);
            }
        }
        // No model takes a "save" command from this module.
        assert!(
            MODELS
                .iter()
                .all(|m| ![0x09, 0x11, 0x59].contains(&m.command[0]))
        );
    }
}
