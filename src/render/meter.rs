//! Stereo level meter: two LED columns with a shared dB scale between them.

use macroquad::prelude::*;

use super::{led_column, Style, Ui, BG, FRAME, TEXT, TEXT_DIM};
use crate::analysis::{level_to_meter, Frame};

const ROWS: usize = 30;
const COLS: usize = 5;
const SCALE_MARKS: [f32; 8] = [0.0, -6.0, -12.0, -20.0, -30.0, -40.0, -50.0, -60.0];

pub(super) fn draw(ui: &Ui, f: &Frame, device: &str) {
    clear_background(BG);
    let r = ui.chrome(Style::Meter, device, f.live);
    let s = ui.s;

    let readout_h = 64.0 * s;
    let top = r.y + 44.0 * s;
    let bottom = r.bottom() - readout_h;
    let spacing = ((bottom - top) / ROWS as f32).max(1.0);
    let block_w = spacing * COLS as f32;
    // The scale sits in the gap between the two columns.
    let gap = 70.0 * s;
    let cx = ui.w / 2.0;

    draw_scale(ui, cx, gap, top, spacing * ROWS as f32);

    let left_l = cx - gap / 2.0 - block_w;
    let left_r = cx + gap / 2.0;
    for (ch, left, name) in [(&f.left, left_l, "L"), (&f.right, left_r, "R")] {
        led_column(&ch.level, left, bottom, spacing, COLS, ROWS);
        let col_cx = left + block_w / 2.0;
        readout(ui, ch.db, f.live, col_cx, bottom + readout_h * 0.4);
        ui.text_centered(name, col_cx, bottom + readout_h * 0.82, 13.0 * s, TEXT_DIM);
    }
}

fn draw_scale(ui: &Ui, cx: f32, gap: f32, top: f32, height: f32) {
    let size = 12.0 * ui.s;
    let tick = 6.0 * ui.s;
    let edge = gap / 2.0 - 4.0 * ui.s;
    for db in SCALE_MARKS {
        let y = top + height * (1.0 - level_to_meter(db));
        draw_line(cx - edge, y, cx - edge + tick, y, 1.0, FRAME);
        draw_line(cx + edge - tick, y, cx + edge, y, 1.0, FRAME);
        ui.text_centered(&format!("{db:.0}"), cx, y, size, TEXT_DIM);
    }
}

fn readout(ui: &Ui, db: f32, live: bool, cx: f32, y: f32) {
    let size = 28.0 * ui.s;
    let (value, color) = if live {
        (format!("{:.1}", db.max(-99.9)), TEXT)
    } else {
        ("-∞".to_string(), TEXT_DIM)
    };
    let unit = " dB";
    let vw = ui.measure(&value, size);
    let uw = ui.measure(unit, size * 0.55);
    let x = cx - (vw + uw) / 2.0;
    ui.text(&value, x, y, size, color);
    ui.text(unit, x + vw, y, size * 0.55, TEXT_DIM);
}
