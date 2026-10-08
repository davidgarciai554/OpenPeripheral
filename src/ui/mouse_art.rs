//! Own schematic drawings of mice for «Botones y DPI» (DESIGN.md, revisión
//! 2026-10-08): a line drawing, no logos, no product photos. Each drawing
//! is data in its own units (a 260 × 420 box, seen from above, front up)
//! with one point per button, in the order of the mouse's profile.
//!
//! Limit: a drawing per model is Rust data. A third model with its own
//! drawing would justify loading them from data files.

/// A path: move, line, cubic Bézier, close.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    M(f32, f32),
    L(f32, f32),
    C(f32, f32, f32, f32, f32, f32),
    Z,
}

/// Where the label of a point goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub at: (f32, f32),
    pub side: Side,
}

pub struct Art {
    pub size: (f32, f32),
    /// Outline of the body (filled with the card colour, then stroked).
    pub body: &'static [Seg],
    /// Lines between the buttons.
    pub seams: &'static [Seg],
    /// Small parts as rounded rectangles: (x, y, w, h, radius).
    pub parts: &'static [(f32, f32, f32, f32, f32)],
    /// One point per button, in profile order.
    pub points: &'static [Point],
}

use Seg::*;
use Side::*;

const fn p(x: f32, y: f32, side: Side) -> Point {
    Point { at: (x, y), side }
}

/// G305: symmetric body, wheel and DPI button on the axis, two side buttons
/// on the left. Button order observed in its profile: left, right, middle,
/// back, forward, DPI (`text::mouse_button`).
pub static G305: Art = Art {
    size: (260.0, 420.0),
    body: &[
        M(130.0, 8.0),
        C(72.0, 8.0, 42.0, 40.0, 36.0, 110.0),
        C(30.0, 180.0, 31.0, 262.0, 40.0, 322.0),
        C(52.0, 392.0, 92.0, 412.0, 130.0, 412.0),
        C(168.0, 412.0, 208.0, 392.0, 220.0, 322.0),
        C(229.0, 262.0, 230.0, 180.0, 224.0, 110.0),
        C(218.0, 40.0, 188.0, 8.0, 130.0, 8.0),
        Z,
    ],
    seams: &[
        M(130.0, 12.0),
        L(130.0, 112.0),
        M(37.0, 150.0),
        C(70.0, 158.0, 104.0, 172.0, 122.0, 188.0),
        M(223.0, 150.0),
        C(190.0, 158.0, 156.0, 172.0, 138.0, 188.0),
    ],
    parts: &[
        (116.0, 36.0, 28.0, 72.0, 12.0),
        (122.0, 44.0, 16.0, 56.0, 8.0),
        (121.0, 118.0, 18.0, 30.0, 8.0),
        (25.0, 150.0, 9.0, 36.0, 4.0),
        (25.0, 190.0, 9.0, 36.0, 4.0),
    ],
    points: &[
        p(80.0, 90.0, Left),
        p(180.0, 90.0, Right),
        p(130.0, 70.0, Top),
        p(30.0, 208.0, Left),
        p(30.0, 168.0, Left),
        p(130.0, 133.0, Right),
    ],
};

/// Any other mouse: a plain body and points for up to 8 buttons in the
/// usual order (left, right, middle, two on the left side, one on top, two
/// on the right side).
pub static GENERIC: Art = Art {
    size: (260.0, 420.0),
    body: &[
        M(130.0, 10.0),
        C(70.0, 10.0, 40.0, 50.0, 38.0, 130.0),
        C(36.0, 220.0, 40.0, 330.0, 70.0, 380.0),
        C(90.0, 410.0, 110.0, 412.0, 130.0, 412.0),
        C(150.0, 412.0, 170.0, 410.0, 190.0, 380.0),
        C(220.0, 330.0, 224.0, 220.0, 222.0, 130.0),
        C(220.0, 50.0, 190.0, 10.0, 130.0, 10.0),
        Z,
    ],
    seams: &[
        M(130.0, 14.0),
        L(130.0, 120.0),
        M(40.0, 160.0),
        C(80.0, 165.0, 110.0, 170.0, 130.0, 172.0),
        C(150.0, 170.0, 180.0, 165.0, 220.0, 160.0),
    ],
    parts: &[(118.0, 40.0, 24.0, 64.0, 10.0)],
    points: &[
        p(80.0, 95.0, Left),
        p(180.0, 95.0, Right),
        p(130.0, 72.0, Top),
        p(40.0, 215.0, Left),
        p(40.0, 175.0, Left),
        p(130.0, 140.0, Right),
        p(220.0, 175.0, Right),
        p(220.0, 215.0, Right),
    ],
};

/// The drawing of a mouse by its name.
pub fn art_for(name: &str) -> &'static Art {
    if name.contains("G305") {
        &G305
    } else {
        &GENERIC
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside(a: &Art, (x, y): (f32, f32)) -> bool {
        (0.0..=a.size.0).contains(&x) && (0.0..=a.size.1).contains(&y)
    }

    #[test]
    fn drawings_cover_their_buttons_and_stay_in_their_box() {
        // The G305 declares 6 buttons; the generic drawing, the most the
        // page shows.
        assert_eq!(G305.points.len(), 6);
        assert!(GENERIC.points.len() >= 6);
        for a in [&G305, &GENERIC] {
            assert!(a.points.iter().all(|p| inside(a, p.at)));
            for s in a.body.iter().chain(a.seams) {
                match *s {
                    M(x, y) | L(x, y) => assert!(inside(a, (x, y))),
                    C(x1, y1, x2, y2, x, y) => {
                        assert!(inside(a, (x1, y1)) && inside(a, (x2, y2)) && inside(a, (x, y)))
                    }
                    Z => {}
                }
            }
            assert!(
                a.parts
                    .iter()
                    .all(|&(x, y, w, h, _)| inside(a, (x, y)) && inside(a, (x + w, y + h)))
            );
            // Points far enough apart to tell them (and their 28 px targets)
            // apart at the smallest drawing.
            for (i, p) in a.points.iter().enumerate() {
                for q in &a.points[i + 1..] {
                    let d = ((p.at.0 - q.at.0).powi(2) + (p.at.1 - q.at.1).powi(2)).sqrt();
                    assert!(d >= 30.0, "{:?} {:?}", p.at, q.at);
                }
            }
        }
        assert!(std::ptr::eq(art_for("G305 Lightspeed"), &G305));
        assert!(std::ptr::eq(art_for("G502 X"), &GENERIC));
    }
}
