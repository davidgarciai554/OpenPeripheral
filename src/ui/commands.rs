//! «Comandos» of the «Asignaciones» panel (DESIGN.md, revisión 2026-10-08):
//! common shortcuts a mouse button can send. Only keys this app can write
//! (`onboard::assignable`), with Ctrl, Mayús, Alt and, in the «Windows»
//! group, the left Windows key (same `80 02 mod usage` encoding with the
//! left GUI bit, 0x08). The user tested them on a G305 on 2026-10-08 and
//! they work. The «Teclas» field does not capture the Windows key (decision
//! of the user): Windows acts on it while typing it.
//!
//! The mouse sends physical keys (HID usages) that Windows reads with the
//! keyboard layout in use. Letters, Tab, F keys, arrows and the numeric
//! keypad are in the same place on every layout, so the zoom uses the
//! keypad's + and − (on a Spanish keyboard «=» is not where «=» is on a US
//! one).

use crate::protocol::onboard::Action;

/// HID modifier bits.
const CTRL: u8 = 0x01;
const SHIFT: u8 = 0x02;
const ALT: u8 = 0x04;
const WIN: u8 = 0x08;

pub struct Command {
    pub group: &'static str,
    pub name: &'static str,
    pub modifiers: u8,
    pub usage: u8,
}

const fn c(group: &'static str, name: &'static str, modifiers: u8, usage: u8) -> Command {
    Command {
        group,
        name,
        modifiers,
        usage,
    }
}

const EDIT: &str = "Edición";
const WEB: &str = "Navegador y pestañas";
const ZOOM: &str = "Zoom";
const WINDOWS: &str = "Ventanas";
const SYSTEM: &str = "Windows";

/// In display order, grouped.
pub const COMMANDS: &[Command] = &[
    c(EDIT, "Copiar", CTRL, 0x06),
    c(EDIT, "Pegar", CTRL, 0x19),
    c(EDIT, "Cortar", CTRL, 0x1b),
    c(EDIT, "Deshacer", CTRL, 0x1d),
    c(EDIT, "Rehacer", CTRL, 0x1c),
    c(EDIT, "Seleccionar todo", CTRL, 0x04),
    c(EDIT, "Guardar", CTRL, 0x16),
    c(EDIT, "Buscar", CTRL, 0x09),
    c(EDIT, "Imprimir", CTRL, 0x13),
    c(EDIT, "Nuevo", CTRL, 0x11),
    c(EDIT, "Abrir", CTRL, 0x12),
    c(WEB, "Nueva pestaña", CTRL, 0x17),
    c(WEB, "Cerrar pestaña", CTRL, 0x1a),
    c(WEB, "Reabrir pestaña", CTRL | SHIFT, 0x17),
    c(WEB, "Pestaña siguiente", CTRL, 0x2b),
    c(WEB, "Pestaña anterior", CTRL | SHIFT, 0x2b),
    c(WEB, "Actualizar", 0, 0x3e),
    c(WEB, "Página anterior", ALT, 0x50),
    c(WEB, "Página siguiente", ALT, 0x4f),
    c(ZOOM, "Acercar", CTRL, 0x57),
    c(ZOOM, "Alejar", CTRL, 0x56),
    c(ZOOM, "Zoom al 100 %", CTRL, 0x62),
    c(WINDOWS, "Cerrar ventana", ALT, 0x3d),
    c(WINDOWS, "Cambiar de ventana", ALT, 0x2b),
    c(SYSTEM, "Mostrar el escritorio", WIN, 0x07),
    c(SYSTEM, "Minimizar todo", WIN, 0x10),
    c(SYSTEM, "Vista de tareas", WIN, 0x2b),
    c(SYSTEM, "Explorador de archivos", WIN, 0x08),
    c(SYSTEM, "Buscar", WIN, 0x16),
    c(SYSTEM, "Configuración", WIN, 0x0c),
    c(SYSTEM, "Configuración rápida", WIN, 0x04),
    c(SYSTEM, "Centro de notificaciones", WIN, 0x11),
    c(SYSTEM, "Bloquear el equipo", WIN, 0x0f),
    c(SYSTEM, "Ejecutar", WIN, 0x15),
    c(SYSTEM, "Menú de inicio rápido", WIN, 0x1b),
    c(SYSTEM, "Historial del portapapeles", WIN, 0x19),
    c(SYSTEM, "Recorte de pantalla", WIN | SHIFT, 0x16),
    c(SYSTEM, "Panel de emojis", WIN, 0x37),
    c(SYSTEM, "Game Bar", WIN, 0x0a),
    c(SYSTEM, "Recorrer la barra de tareas", WIN, 0x17),
    c(SYSTEM, "Área de notificación", WIN, 0x05),
    c(SYSTEM, "Proyectar", WIN, 0x13),
    c(SYSTEM, "Narrador", WIN | CTRL, 0x28),
    c(SYSTEM, "Nuevo escritorio virtual", WIN | CTRL, 0x07),
    c(SYSTEM, "Escritorio virtual siguiente", WIN | CTRL, 0x4f),
    c(SYSTEM, "Escritorio virtual anterior", WIN | CTRL, 0x50),
];

impl Command {
    pub fn action(&self) -> Action {
        Action::Keyboard {
            modifiers: self.modifiers,
            usage: self.usage,
        }
    }

    /// "Ctrl + V".
    pub fn keys(&self) -> String {
        super::text::shortcut(self.modifiers, self.usage)
    }
}

/// Lower case, without accents, for the search.
fn fold(s: &str) -> String {
    s.chars()
        .map(|ch| match ch {
            'á' | 'à' | 'ä' | 'Á' | 'À' | 'Ä' => 'a',
            'é' | 'è' | 'ë' | 'É' | 'È' | 'Ë' => 'e',
            'í' | 'ì' | 'ï' | 'Í' | 'Ì' | 'Ï' => 'i',
            'ó' | 'ò' | 'ö' | 'Ó' | 'Ò' | 'Ö' => 'o',
            'ú' | 'ù' | 'ü' | 'Ú' | 'Ù' | 'Ü' => 'u',
            'ñ' | 'Ñ' => 'n',
            other => other.to_lowercase().next().unwrap_or(other),
        })
        .collect()
}

/// Indexes of the commands whose name, keys or group contain `query`
/// (ignoring case and accents); all of them for an empty query.
pub fn filter(query: &str) -> Vec<usize> {
    let q = fold(query.trim());
    COMMANDS
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            q.is_empty() || fold(&format!("{} {} {}", c.name, c.keys(), c.group)).contains(&q)
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::onboard::assignable;

    #[test]
    fn every_command_can_be_written_and_reads_back() {
        for c in COMMANDS {
            let a = c.action();
            assert!(assignable(a), "{}", c.name);
            assert_eq!(Action::decode(a.encode()), a, "{}", c.name);
            // Only the left Windows key, and only in its group.
            assert_eq!(c.modifiers & 0xf0, 0, "{}", c.name);
            assert_eq!(c.modifiers & WIN != 0, c.group == SYSTEM, "{}", c.name);
            assert!(!c.keys().contains("Tecla 0x"), "{}: {}", c.name, c.keys());
        }
        let mut seen = std::collections::HashSet::new();
        assert!(COMMANDS.iter().all(|c| seen.insert((c.modifiers, c.usage))));
    }

    #[test]
    fn search_ignores_case_and_accents() {
        assert_eq!(filter("").len(), COMMANDS.len());
        assert_eq!(filter("  ").len(), COMMANDS.len());
        let pegar = filter("PEGAR");
        assert_eq!(pegar.len(), 1);
        assert_eq!(COMMANDS[pegar[0]].keys(), "Ctrl + V");
        // «pestana» finds «Nueva pestaña»…; keys and groups match too.
        assert!(filter("pestana").len() >= 4);
        assert!(filter("ctrl + v").contains(&pegar[0]));
        assert_eq!(filter("zoom").len(), 3);
        assert_eq!(filter("windows").len(), 22);

        assert!(filter("xyzzy").is_empty());
    }

    #[test]
    fn keypad_keys_have_names() {
        let acercar = COMMANDS.iter().find(|c| c.name == "Acercar").unwrap();
        assert_eq!(acercar.keys(), "Ctrl + + (teclado numérico)");
    }
}
