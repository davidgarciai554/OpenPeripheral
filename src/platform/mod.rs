//! Operating-system specific code.

pub mod hid;
pub mod system;

/// Lets CLI output reach the terminal that started this GUI-subsystem binary.
pub fn attach_console() {
    use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}
