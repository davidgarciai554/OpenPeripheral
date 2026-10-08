// No console window when started normally; the CLI attaches to the parent's.
#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod backup;
mod config;
mod devices;
mod games;
mod platform;
mod profiles;
mod protocol;
mod ui;

use devices::{Change, Device};
use platform::hid::{HidTransport, scan};
use std::process::ExitCode;

const USAGE: &str = "\
OpenPeripheral: control ligero de periféricos, sin cuentas ni conexión a internet.

Sin argumentos inicia el icono de la bandeja del sistema.

  --list                    lista los dispositivos compatibles y su estado
  --apply <n> dpi=<valor>   cambia el DPI del dispositivo n
  --apply <n> color=#RRGGBB cambia el color del dispositivo n

Los cambios por línea de órdenes no se guardan en el dispositivo ni en la
configuración.";

fn parse_change(s: &str) -> Option<Change> {
    let (k, v) = s.split_once('=')?;
    match k {
        "dpi" => v.parse().ok().map(Change::Dpi),
        "color" => config::parse_hex(v).map(Change::Color),
        _ => None,
    }
}

fn describe(d: &mut Box<dyn Device>, api: &hidapi::HidApi) -> String {
    match HidTransport::open(api, d.path()).and_then(|t| d.read_state(&t)) {
        Ok(s) => ui::text::state_lines(&s).join(" · "),
        Err(e) => format!("sin datos: {e}"),
    }
}

fn cli(args: &[String]) -> ExitCode {
    let mut api = match hidapi::HidApi::new() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("No se pudo iniciar el acceso HID: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut devs = scan(&mut api);
    match args.first().map(String::as_str) {
        Some("--list") => {
            if devs.is_empty() {
                println!("No se encontraron dispositivos compatibles.");
            }
            for (i, d) in devs.iter_mut().enumerate() {
                let line = describe(d, &api);
                println!("[{i}] {} {} — {line}", d.info().vendor, d.info().name);
            }
            ExitCode::SUCCESS
        }
        Some("--apply") => {
            let (Some(i), Some(change)) = (
                args.get(1).and_then(|s| s.parse::<usize>().ok()),
                args.get(2).and_then(|s| parse_change(s)),
            ) else {
                eprintln!("{USAGE}");
                return ExitCode::FAILURE;
            };
            let Some(d) = devs.get_mut(i) else {
                eprintln!("No existe el dispositivo [{i}]. Usa --list.");
                return ExitCode::FAILURE;
            };
            match HidTransport::open(&api, d.path()).and_then(|t| d.apply(&t, change)) {
                Ok(()) => {
                    println!("Aplicado a {}.", d.info().name);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("No se pudo aplicar: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        ui::run();
        return ExitCode::SUCCESS;
    }
    platform::attach_console();
    cli(&args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::razer::Rgb;

    #[test]
    fn change_parsing() {
        assert_eq!(parse_change("dpi=1600"), Some(Change::Dpi(1600)));
        assert_eq!(
            parse_change("color=#ff0000"),
            Some(Change::Color(Rgb { r: 255, g: 0, b: 0 }))
        );
        assert_eq!(parse_change("hz=500"), None);
        assert_eq!(parse_change("dpi=abc"), None);
        assert_eq!(parse_change("dpi"), None);
    }
}
