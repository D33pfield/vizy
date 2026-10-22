//! LED dot-matrix spectrum: one column of dots per band, coloured by height,
//! with peak-hold dots and a frequency scale underneath.

use macroquad::prelude::*;

use super::{freq_axis, led_column, Style, Ui, BG, TEXT_DIM};
use crate::analysis::Frame;

const ROWS: usize = 24;

pub(super) fn draw(ui: &Ui, f: &Frame, device: &str) {
    clear_background(BG);
    let r = ui.chrome(Style::Matrix, device, f.live);
    let s = ui.s;

    // Plot area inside the frame, leaving room for frequency labels below.
    let area = Rect::new(r.x + 28.0 * s, r.y + 36.0 * s, r.w - 56.0 * s, r.h - 80.0 * s);
    let n = f.mono.len();
    let cell = (area.w / n as f32).min(area.h / ROWS as f32);
    let grid_w = cell * n as f32;
    let left = area.x + (area.w - grid_w) / 2.0;
    let bottom = area.y + (area.h + cell * ROWS as f32) / 2.0;

    for (col, band) in f.mono.iter().enumerate() {
        led_column(band, left + col as f32 * cell, bottom, cell, 1, ROWS);
    }

    freq_axis(ui, f, left, grid_w, bottom + 16.0 * s, 12.0 * s, TEXT_DIM);
}
