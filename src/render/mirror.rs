//! Mirrored spectrum: square segments growing out of a centre line, left
//! channel upward and right channel downward, coloured by distance from the
//! centre, with floating peak squares. No frame; just the bars on black.

use macroquad::prelude::*;

use super::{ramp, with_alpha, Ui, HINT, TEXT_DIM};
use crate::analysis::Frame;

/// Segments on each side of the centre line.
const HALF: usize = 14;
const BG: Color = Color::new(0.0, 0.0, 0.0, 1.0);
const RAMP: [(f32, Color); 5] = [
    (0.0, Color::new(0.15, 0.45, 1.00, 1.0)),
    (0.3, Color::new(0.00, 0.80, 0.75, 1.0)),
    (0.55, Color::new(0.45, 0.95, 0.20, 1.0)),
    (0.8, Color::new(1.00, 0.60, 0.10, 1.0)),
    (1.0, Color::new(1.00, 0.15, 0.40, 1.0)),
];

pub(super) fn draw(ui: &Ui, f: &Frame) {
    clear_background(BG);
    let s = ui.s;
    let footer_h = 30.0 * s;

    let n = f.left.bands.len();
    let cell = (ui.w * 0.94 / n as f32).min((ui.h - footer_h) * 0.9 / (2 * HALF + 1) as f32);
    let left = (ui.w - cell * n as f32) / 2.0;
    let cy = (ui.h - footer_h) / 2.0;
    let (seg_w, seg_h) = (cell * 0.7, cell * 0.8);

    for col in 0..n {
        let x = left + col as f32 * cell + (cell - seg_w) / 2.0;
        // The centre square is always on, so silence still shows a baseline.
        segment(x, cy, seg_w, seg_h, RAMP[0].1, 1.0);

        for (band, dir) in [(&f.left.bands[col], -1.0), (&f.right.bands[col], 1.0)] {
            let lit = band.value * HALF as f32;
            for i in 1..=HALF {
                let amount = (lit - (i - 1) as f32).clamp(0.0, 1.0);
                if amount > 0.0 {
                    let color = ramp(&RAMP, i as f32 / HALF as f32);
                    segment(x, cy + dir * i as f32 * cell, seg_w, seg_h, color, amount);
                }
            }
            // A dimmer square floats at the peak while the bar falls away.
            let peak = (band.peak * HALF as f32).ceil() as usize;
            if peak >= 1 && peak as f32 > lit.ceil() {
                let color = ramp(&RAMP, peak as f32 / HALF as f32);
                segment(x, cy + dir * peak as f32 * cell, seg_w, seg_h, color, 0.55);
            }
        }
    }

    let y = ui.h - footer_h / 2.0;
    let dim = with_alpha(TEXT_DIM, 0.6);
    ui.text("vizy · mirror   L ▲  R ▼", 16.0 * s, y, 13.0 * s, dim);
    ui.text_right(HINT, ui.w - 16.0 * s, y, 13.0 * s, dim);
}

fn segment(x: f32, cy: f32, w: f32, h: f32, color: Color, alpha: f32) {
    draw_rectangle(x, cy - h / 2.0, w, h, with_alpha(color, alpha));
}
