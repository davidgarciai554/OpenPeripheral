//! Pure protocol codecs. No I/O: every function maps bytes to typed values
//! and back, so it can be tested without hardware.

pub mod asus;
pub mod dualsense;
pub mod hidpp;
pub mod onboard;
pub mod razer;
pub mod steelseries;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeState {
    Discharging,
    Charging,
    Full,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Battery {
    pub percent: u8,
    pub charging: ChargeState,
}

/// Supported DPI values as declared by the sensor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DpiRange {
    Range { min: u16, max: u16, step: u16 },
    Values(Vec<u16>),
}

impl DpiRange {
    pub fn contains(&self, dpi: u16) -> bool {
        match self {
            DpiRange::Range { min, max, step } => {
                dpi >= *min && dpi <= *max && *step > 0 && (dpi - min) % step == 0
            }
            DpiRange::Values(v) => v.contains(&dpi),
        }
    }
}
