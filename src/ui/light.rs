//! Pure logic of the "Iluminación" section (`docs/design/DESIGN.md`): what
//! the user picked, its texts and the light it stands for. No Win32 here.

use crate::config::{parse_hex, to_hex};
use crate::protocol::onboard::{BREATHING_PERIODS, CYCLE_PERIODS, Light};
use crate::protocol::razer::Rgb;

/// Initial swatches. A plain data list: adding one only changes this table
/// (the row wraps).
pub const SWATCHES: [(&str, u32); 8] = [
    ("Blanco", 0xFFFFFF),
    ("Rojo", 0xFF0000),
    ("Naranja", 0xFF8000),
    ("Amarillo", 0xFFD000),
    ("Verde", 0x00FF40),
    ("Cian", 0x00C8FF),
    ("Azul", 0x0040FF),
    ("Morado", 0xA000FF),
];

/// Effects in display order, with the id the mouse declares (0x8070).
pub const EFFECTS: [(u16, &str); 4] = [
    (0x00, "Apagada"),
    (0x01, "Fija"),
    (0x03, "Ciclo de colores"),
    (0x0a, "Respiración"),
];

/// Speed names, slow to fast (same order as the period tables).
pub const SPEEDS: [&str; 3] = ["Lenta", "Media", "Rápida"];

pub fn swatch(i: usize) -> Rgb {
    let v = SWATCHES[i].1;
    Rgb {
        r: (v >> 16) as u8,
        g: (v >> 8) as u8,
        b: v as u8,
    }
}

/// "Cian · #00C8FF" or "Personalizado · #12AB34".
pub fn color_name(c: Rgb) -> String {
    let name = (0..SWATCHES.len())
        .find(|&i| swatch(i) == c)
        .map_or("Personalizado", |i| SWATCHES[i].0);
    format!("{name} · {}", to_hex(c))
}

/// What the user picked. Color and speed are kept while the effect does not
/// use them, so going back to "Fija" restores the previous color.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    pub effect: u16,
    /// Text of the Hex field, as typed.
    pub hex: String,
    pub speed: usize,
}

impl Pick {
    /// Starts from a light read from the mouse or the app's settings.
    pub fn from_light(l: Light) -> Self {
        let (color, speed) = match l {
            Light::Off => (None, 1),
            Light::Fixed(c) => (Some(c), 1),
            Light::Cycle { period_ms } => (None, speed_of(&CYCLE_PERIODS, period_ms)),
            Light::Breathing { color, period_ms } => {
                (Some(color), speed_of(&BREATHING_PERIODS, period_ms))
            }
        };
        Pick {
            effect: l.effect_id(),
            hex: to_hex(color.unwrap_or(swatch(5))),
            speed,
        }
    }

    pub fn uses_color(&self) -> bool {
        matches!(self.effect, 0x01 | 0x0a)
    }

    pub fn uses_speed(&self) -> bool {
        matches!(self.effect, 0x03 | 0x0a)
    }

    pub fn color(&self) -> Option<Rgb> {
        parse_hex(&self.hex)
    }

    /// The light this pick stands for; `None` while the Hex field is invalid.
    pub fn light(&self) -> Option<Light> {
        let speed = self.speed.min(SPEEDS.len() - 1);
        Some(match self.effect {
            0x00 => Light::Off,
            0x01 => Light::Fixed(self.color()?),
            0x03 => Light::Cycle {
                period_ms: CYCLE_PERIODS[speed],
            },
            0x0a => Light::Breathing {
                color: self.color()?,
                period_ms: BREATHING_PERIODS[speed],
            },
            _ => return None,
        })
    }
}

/// Closest speed for a period (a light saved by other software may use one
/// that is not in the table).
fn speed_of(table: &[u16; 3], period_ms: u16) -> usize {
    (0..table.len())
        .min_by_key(|&i| table[i].abs_diff(period_ms))
        .unwrap_or(1)
}

fn speed_word(table: &[u16; 3], period_ms: u16) -> String {
    match table.iter().position(|&p| p == period_ms) {
        Some(i) => SPEEDS[i].to_lowercase(),
        None => format!("{} s", period_ms as f32 / 1000.0).replace('.', ","),
    }
}

/// "apagada", "fija, Cian #00C8FF", "ciclo de colores, media",
/// "respiración, Naranja #FF8000, lenta".
pub fn describe(l: Light) -> String {
    let named = |c: Rgb| color_name(c).replace(" · ", " ");
    match l {
        Light::Off => "apagada".into(),
        Light::Fixed(c) => format!("fija, {}", named(c)),
        Light::Cycle { period_ms } => {
            format!(
                "ciclo de colores, {}",
                speed_word(&CYCLE_PERIODS, period_ms)
            )
        }
        Light::Breathing { color, period_ms } => format!(
            "respiración, {}, {}",
            named(color),
            speed_word(&BREATHING_PERIODS, period_ms)
        ),
    }
}

/// Same text with a capital letter, for "Guardado: …".
pub fn describe_cap(l: Light) -> String {
    let d = describe(l);
    let mut c = d.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// "Error: «#00C8F» no es un color válido…" for an invalid Hex field.
pub fn invalid_hex(typed: &str) -> String {
    format!(
        "Error: «{}» no es un color válido. Escribe 6 cifras hexadecimales, por ejemplo #00C8FF.",
        typed.trim()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgb = Rgb { r: 255, g: 0, b: 0 };

    #[test]
    fn names_and_descriptions() {
        assert_eq!(color_name(swatch(5)), "Cian · #00C8FF");
        assert_eq!(
            color_name(Rgb {
                r: 0x12,
                g: 0xab,
                b: 0x34
            }),
            "Personalizado · #12AB34"
        );
        assert_eq!(describe(Light::Off), "apagada");
        assert_eq!(describe(Light::Fixed(RED)), "fija, Rojo #FF0000");
        assert_eq!(
            describe(Light::Cycle { period_ms: 8000 }),
            "ciclo de colores, media"
        );
        assert_eq!(
            describe(Light::Breathing {
                color: RED,
                period_ms: 6000
            }),
            "respiración, Rojo #FF0000, lenta"
        );
        // A period outside the tables is shown in seconds.
        assert_eq!(
            describe(Light::Cycle { period_ms: 2500 }),
            "ciclo de colores, 2,5 s"
        );
        assert_eq!(describe_cap(Light::Off), "Apagada");
    }

    #[test]
    fn pick_round_trips_every_table_light() {
        let mut lights = vec![Light::Off, Light::Fixed(RED)];
        lights.extend(CYCLE_PERIODS.map(|p| Light::Cycle { period_ms: p }));
        lights.extend(BREATHING_PERIODS.map(|p| Light::Breathing {
            color: RED,
            period_ms: p,
        }));
        for l in lights {
            assert_eq!(Pick::from_light(l).light(), Some(l), "{l:?}");
            assert!(l.writable());
        }
    }

    #[test]
    fn pick_validates_the_hex_field() {
        let mut p = Pick::from_light(Light::Fixed(RED));
        for (typed, ok) in [
            ("#00C8FF", true),
            ("00c8ff", true),
            (" #00C8FF ", true),
            ("#00C8F", false),
            ("", false),
            ("#GG0000", false),
        ] {
            p.hex = typed.into();
            assert_eq!(p.light().is_some(), ok, "{typed}");
        }
        // Effects without color do not depend on the field.
        p.effect = 0x03;
        assert!(p.light().is_some());
        assert!(!p.uses_color() && p.uses_speed());
        // Switching from cycle back to fixed keeps the last color.
        let mut p = Pick::from_light(Light::Fixed(RED));
        p.effect = 0x03;
        p.effect = 0x01;
        assert_eq!(p.light(), Some(Light::Fixed(RED)));
        // Off and cycle start from Cian and medium speed.
        let p = Pick::from_light(Light::Off);
        assert_eq!((p.hex.as_str(), p.speed), ("#00C8FF", 1));
        assert!(invalid_hex("#00C8F").starts_with("Error: «#00C8F» no es"));
    }
}
