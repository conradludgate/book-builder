//! Utility / conveniece functions for commonly use graphical shapes

use std::cell::RefCell;

use base64::Engine;
use nanorand::Rng;

use crate::printpdf::scale::Pt;
use crate::printpdf::Point;

// PDF doesn't understand what a "circle" is, so we have to
// approximate it.
const C: f32 = 0.55191505;

/// Calculates and returns the points for an approximated circle, given a radius and an
/// offset into the centre of circle (starting from bottom left corner of page).
#[inline]
pub fn calculate_points_for_circle<P: Into<Pt>>(
    radius: P,
    offset_x: P,
    offset_y: P,
) -> Vec<(Point, bool)> {
    let (radius, offset_x, offset_y) = (radius.into(), offset_x.into(), offset_y.into());
    let radius = radius.0;

    let p10 = Point {
        x: Pt(0.0 * radius),
        y: Pt(1.0 * radius),
    };
    let p11 = Point {
        x: Pt(C * radius),
        y: Pt(1.0 * radius),
    };
    let p12 = Point {
        x: Pt(1.0 * radius),
        y: Pt(C * radius),
    };
    let p13 = Point {
        x: Pt(1.0 * radius),
        y: Pt(0.0 * radius),
    };

    let p20 = Point {
        x: Pt(1.0 * radius),
        y: Pt(0.0 * radius),
    };
    let p21 = Point {
        x: Pt(1.0 * radius),
        y: Pt(-C * radius),
    };
    let p22 = Point {
        x: Pt(C * radius),
        y: Pt(-1.0 * radius),
    };
    let p23 = Point {
        x: Pt(0.0 * radius),
        y: Pt(-1.0 * radius),
    };

    let p30 = Point {
        x: Pt(0.0 * radius),
        y: Pt(-1.0 * radius),
    };
    let p31 = Point {
        x: Pt(-C * radius),
        y: Pt(-1.0 * radius),
    };
    let p32 = Point {
        x: Pt(-1.0 * radius),
        y: Pt(-C * radius),
    };
    let p33 = Point {
        x: Pt(-1.0 * radius),
        y: Pt(0.0 * radius),
    };

    let p40 = Point {
        x: Pt(-1.0 * radius),
        y: Pt(0.0 * radius),
    };
    let p41 = Point {
        x: Pt(-1.0 * radius),
        y: Pt(C * radius),
    };
    let p42 = Point {
        x: Pt(-C * radius),
        y: Pt(1.0 * radius),
    };
    let p43 = Point {
        x: Pt(0.0 * radius),
        y: Pt(1.0 * radius),
    };

    let mut pts = vec![
        (p10, true),
        (p11, true),
        (p12, true),
        (p13, false),
        (p20, true),
        (p21, true),
        (p22, true),
        (p23, false),
        (p30, true),
        (p31, true),
        (p32, true),
        (p33, false),
        (p40, true),
        (p41, true),
        (p42, true),
        (p43, false),
    ];

    for &mut (ref mut p, _) in pts.iter_mut() {
        p.x.0 += offset_x.0;
        p.y.0 += offset_y.0;
    }

    pts
}

/// Calculates and returns the points for a rectangle, given a horizontal and vertical scale.
/// and an offset into the centre of rectangle (starting from bottom left corner of page).
#[inline]
pub fn calculate_points_for_rect<P: Into<Pt>>(
    scale_x: P,
    scale_y: P,
    offset_x: P,
    offset_y: P,
) -> Vec<(Point, bool)> {
    let (scale_x, scale_y, offset_x, offset_y) = (
        scale_x.into(),
        scale_y.into(),
        offset_x.into(),
        offset_y.into(),
    );
    let top = Pt(offset_y.0 + (scale_y.0 / 2.0));
    let bottom = Pt(offset_y.0 - (scale_y.0 / 2.0));
    let left = Pt(offset_x.0 - (scale_x.0 / 2.0));
    let right = Pt(offset_x.0 + (scale_x.0 / 2.0));

    let top_left_pt = Point { x: left, y: top };
    let top_right_pt = Point { x: right, y: top };
    let bottom_right_pt = Point {
        x: right,
        y: bottom,
    };
    let bottom_left_pt = Point { x: left, y: bottom };

    vec![
        (top_left_pt, false),
        (top_right_pt, false),
        (bottom_right_pt, false),
        (bottom_left_pt, false),
    ]
}

thread_local! {
    static RAND: RefCell<nanorand::WyRand> = RefCell::new(nanorand::WyRand::new());
}

/// Returns a string with 32 random characters
pub(crate) fn random_character_string_32() -> String {
    let mut bytes = [0; 24];
    RAND.with(|x| x.borrow_mut().fill_bytes(&mut bytes[..]));
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
