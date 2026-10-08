//! Windows toast with buttons for an app without installer (plan
//! `perfiles-por-juego`, D12; spike passed on 2026-10-07).
//!
//! The app is identified by an AUMID registered in
//! `HKCU\Software\Classes\AppUserModelId\OpenPeripheral` (DisplayName only,
//! no COM activator), so clicks only reach the app while it is running.

use std::cell::RefCell;
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{
    ToastActivatedEventArgs, ToastNotification, ToastNotificationManager,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegOpenKeyExW, RegSetValueExW,
};
use windows::core::{HSTRING, IInspectable, Interface, w};

pub const AUMID: &str = "OpenPeripheral";

thread_local! {
    // A toast must stay alive for its Activated handler to run.
    static SHOWN: RefCell<Vec<ToastNotification>> = const { RefCell::new(Vec::new()) };
}

/// Creates the AUMID key with the name Windows shows on the toast.
pub fn register() -> windows::core::Result<()> {
    unsafe {
        let mut key = HKEY::default();
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Classes\\AppUserModelId\\OpenPeripheral"),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
        .ok()?;
        let name: Vec<u16> = "OpenPeripheral".encode_utf16().chain([0]).collect();
        let bytes = std::slice::from_raw_parts(name.as_ptr().cast::<u8>(), name.len() * 2);
        let r = RegSetValueExW(key, w!("DisplayName"), None, REG_SZ, Some(bytes));
        let _ = RegCloseKey(key);
        r.ok()
    }
}

/// Whether the AUMID key exists (it is created with the first toast).
pub fn registered() -> bool {
    unsafe {
        let mut key = HKEY::default();
        let ok = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Classes\\AppUserModelId\\OpenPeripheral"),
            None,
            KEY_QUERY_VALUE,
            &mut key,
        )
        .is_ok();
        if ok {
            let _ = RegCloseKey(key);
        }
        ok
    }
}

/// Removes the AUMID key ("Quitar avisos de Windows").
pub fn unregister() -> windows::core::Result<()> {
    unsafe {
        RegDeleteTreeW(
            HKEY_CURRENT_USER,
            w!("Software\\Classes\\AppUserModelId\\OpenPeripheral"),
        )
        .ok()
    }
}

/// Shows a toast with `title`, `body` and `buttons` (label, argument). The
/// texts go into the XML through the DOM, never by formatting markup.
/// `clicked` gets the argument of the button (or `launch` for the body).
pub fn show(
    title: &str,
    body: &str,
    launch: &str,
    buttons: &[(&str, &str)],
    clicked: impl Fn(String) + Send + 'static,
) -> windows::core::Result<()> {
    let doc = XmlDocument::new()?;
    let toast = doc.CreateElement(&HSTRING::from("toast"))?;
    // What a click on the toast itself (not a button) reports.
    toast.SetAttribute(&HSTRING::from("launch"), &HSTRING::from(launch))?;
    doc.AppendChild(&toast)?;
    let visual = doc.CreateElement(&HSTRING::from("visual"))?;
    toast.AppendChild(&visual)?;
    let binding = doc.CreateElement(&HSTRING::from("binding"))?;
    binding.SetAttribute(&HSTRING::from("template"), &HSTRING::from("ToastGeneric"))?;
    visual.AppendChild(&binding)?;
    for t in [title, body] {
        let e = doc.CreateElement(&HSTRING::from("text"))?;
        e.AppendChild(&doc.CreateTextNode(&HSTRING::from(t))?)?;
        binding.AppendChild(&e)?;
    }
    let actions = doc.CreateElement(&HSTRING::from("actions"))?;
    toast.AppendChild(&actions)?;
    for (label, arg) in buttons {
        let a = doc.CreateElement(&HSTRING::from("action"))?;
        a.SetAttribute(&HSTRING::from("content"), &HSTRING::from(*label))?;
        a.SetAttribute(&HSTRING::from("arguments"), &HSTRING::from(*arg))?;
        a.SetAttribute(
            &HSTRING::from("activationType"),
            &HSTRING::from("foreground"),
        )?;
        actions.AppendChild(&a)?;
    }
    let n = ToastNotification::CreateToastNotification(&doc)?;
    n.Activated(&TypedEventHandler::<ToastNotification, IInspectable>::new(
        move |_, args| {
            let arg = args
                .as_ref()
                .and_then(|a| a.cast::<ToastActivatedEventArgs>().ok())
                .and_then(|e| e.Arguments().ok())
                .map(|h| h.to_string())
                .unwrap_or_default();
            clicked(arg);
            Ok(())
        },
    ))?;
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(AUMID))?.Show(&n)?;
    SHOWN.with(|s| s.borrow_mut().push(n));
    Ok(())
}
