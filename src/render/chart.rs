//! btop-style chart: two stacked panels (left and right channel) of blocky
//! spectrum bars on a magenta grid, with peak blocks and amber axis labels.

use macroquad::prelude::*;

use super::{freq_axis, with_alpha, Ui, HINT, TEXT};
use crate::analysis::{Frame, Smoothed, SPECTRUM_RANGE_DB};

const ROWS: usize = 20;
const BG: Color = Color::new(0.04, 0.04, 0.045, 1.0);
const BORDER: Color = Color::new(0.85, 0.62, 0.20, 1.0);
const TITLE: Color = Color::new(0.30, 0.80, 0.35, 1.0);
const GRID: Color = Color::new(0.50, 0.10, 0.45, 0.6);
const BAR: Color = Color::new(0.23, 0.59, 0.87, 1.0);
const PEAK: Color = Color::new(0.60, 0.82, 1.00, 1.0);

pub(super) fn draw(ui: &Ui, f: &Frame, device: &str) {
    clear_background(BG);
    let s = ui.s;
    let m = 12.0 * s;
    let gap = 14.0 * s;
    let ph = (ui.h - m * 2.0 - gap) / 2.0;

    let top = Rect::new(m, m, ui.w - m * 2.0, ph);
    let bottom = Rect::new(m, m + ph + gap, ui.w - m * 2.0, ph);
    let status = if f.live { device.to_string() } else { format!("{device} (idle)") };
    panel(ui, f, top, "L", "left channel", &f.left.bands, &status);
    panel(ui, f, bottom, "R", "right channel", &f.right.bands, HINT);
}

fn panel(ui: &Ui, f: &Frame, r: Rect, tag: &str, desc: &str, bands: &[Smoothed], corner: &str) {
    let s = ui.s;
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5 * s, BORDER);

    // Labels set into the top border: "L ─── SPECTRUM left channel ─── <corner>".
    let size = 14.0 * s;
    let x = r.x + 10.0 * s;
    ui.notch(x, r.y, ui.measure(tag, size), BG);
    ui.text(tag, x, r.y, size, TITLE);
    let title = "SPECTRUM ";
    let x = r.x + 70.0 * s;
    let tw = ui.measure(title, size);
    ui.notch(x, r.y, tw + ui.measure(desc, size), BG);
    ui.text(title, x, r.y, size, TITLE);
    ui.text(desc, x + tw, r.y, size, TEXT);
    let cw = ui.measure(corner, size * 0.9);
    ui.notch(r.right() - 12.0 * s - cw, r.y, cw, BG);
    ui.text(corner, r.right() - 12.0 * s - cw, r.y, size * 0.9, with_alpha(TEXT, 0.7));

    // Plot area: y-axis labels on the left, frequency labels underneath.
    let axis_w = 44.0 * s;
    let plot = Rect::new(r.x + axis_w, r.y + 14.0 * s, r.w - axis_w - 10.0 * s, r.h - 38.0 * s);
    let n = bands.len();
    let (cell_w, cell_h) = (plot.w / n as f32, plot.h / ROWS as f32);

    for i in 0..=n {
        let x = plot.x + i as f32 * cell_w;
        draw_line(x, plot.y, x, plot.bottom(), 1.0, GRID);
    }
    for j in 0..=ROWS {
        let y = plot.y + j as f32 * cell_h;
        draw_line(plot.x, y, plot.right(), y, 1.0, GRID);
    }

    // Bars snap to whole grid cells for the blocky terminal look.
    for (i, band) in bands.iter().enumerate() {
        let x = plot.x + i as f32 * cell_w;
        let cells = (band.value * ROWS as f32).round();
        if cells > 0.0 {
            draw_rectangle(x, plot.bottom() - cells * cell_h, cell_w, cells * cell_h, BAR);
        }
        let peak = (band.peak * ROWS as f32).round();
        if peak > cells {
            let y = plot.bottom() - peak * cell_h;
            draw_rectangle(x, y + cell_h * 0.2, cell_w, cell_h * 0.6, PEAK);
        }
    }

    // Y axis in dB relative to the current auto-gain level.
    let label = 11.0 * s;
    for j in (0..=ROWS).step_by(4) {
        let db = f.gain_top_db - SPECTRUM_RANGE_DB * (1.0 - j as f32 / ROWS as f32);
        let y = plot.bottom() - j as f32 * cell_h;
        ui.text_right(&format!("{db:.0}"), plot.x - 6.0 * s, y, label, BORDER);
    }
    freq_axis(ui, f, plot.x, plot.w, plot.bottom() + 11.0 * s, label, BORDER);
}
