//! Drawing. Styles only read the analysis `Frame`; they never touch audio
//! directly, so new looks (a VFD panel later) slot in over the same data.

mod chart;
mod horizon;
mod matrix;
mod meter;
mod mirror;

use macroquad::prelude::*;

use crate::analysis::{Frame, Smoothed};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Meter,
    Matrix,
    Mirror,
    Chart,
    Horizon,
}

impl Style {
    pub const ALL: [Style; 5] =
        [Style::Meter, Style::Matrix, Style::Mirror, Style::Chart, Style::Horizon];

    pub fn name(self) -> &'static str {
        match self {
            Style::Meter => "meter",
            Style::Matrix => "matrix",
            Style::Mirror => "mirror",
            Style::Chart => "chart",
            Style::Horizon => "horizon",
        }
    }

    /// How many spectrum bands this style wants from the analyzer.
    pub fn bands(self) -> usize {
        match self {
            Style::Meter => 32, // unused, but the analyzer always needs some
            Style::Matrix => 40,
            Style::Mirror => 48,
            Style::Chart => 96,
            Style::Horizon => 40, // per channel; the ridge shows both
        }
    }

    /// Lowest frequency shown. Styles with many bands reach down into the sub-bass.
    pub fn freq_lo(self) -> f32 {
        match self {
            Style::Meter | Style::Matrix => 40.0,
            Style::Mirror | Style::Horizon => 25.0,
            Style::Chart => 20.0,
        }
    }

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&s| s == self).unwrap();
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

pub(super) const BG: Color = Color::new(0.027, 0.031, 0.043, 1.0);
pub(super) const FRAME: Color = Color::new(0.20, 0.23, 0.29, 1.0);
pub(super) const TEXT_DIM: Color = Color::new(0.42, 0.46, 0.54, 1.0);
pub(super) const TEXT: Color = Color::new(0.78, 0.82, 0.88, 1.0);
const ACCENT: Color = Color::new(0.35, 0.85, 0.95, 1.0);
const LIVE: Color = Color::new(0.30, 0.95, 0.45, 1.0);

pub(super) const HINT: &str = "tab/1-5 style · q quit";

/// Labels for frequency axes, like the ones printed under a hi-fi EQ.
const FREQ_LABELS: [(f32, &str); 9] = [
    (20.0, "20"),
    (31.5, "31"),
    (63.0, "63"),
    (160.0, "160"),
    (400.0, "400"),
    (1000.0, "1k"),
    (2500.0, "2.5k"),
    (6300.0, "6.3k"),
    (16000.0, "16k"),
];

/// Classic LED meter colours: green → amber → red, bottom to top.
pub(super) const LED_RAMP: [(f32, Color); 3] = [
    (0.0, Color::new(0.15, 0.90, 0.45, 1.0)),
    (0.65, Color::new(1.00, 0.82, 0.20, 1.0)),
    (1.0, Color::new(1.00, 0.25, 0.25, 1.0)),
];

/// Monospace font from the Windows install; falls back to macroquad's built-in one.
const FONT_PATHS: [&str; 2] = [
    "C:\\Windows\\Fonts\\CascadiaMono.ttf",
    "C:\\Windows\\Fonts\\consola.ttf",
];

/// How long the style name shows after switching.
const TOAST_SECONDS: f32 = 1.2;

pub struct Renderer {
    // The scene is drawn offscreen first so a glow/bloom pass can be added
    // later between the scene and the screen.
    target: RenderTarget,
    font: Option<Font>,
    toast: f32,
    horizon: horizon::Horizon,
}

impl Renderer {
    pub fn new() -> Self {
        let font = FONT_PATHS
            .iter()
            .find_map(|p| std::fs::read(p).ok())
            .and_then(|bytes| load_ttf_font_from_bytes(&bytes).ok());
        Self { target: make_target(), font, toast: 0.0, horizon: horizon::Horizon::new() }
    }

    /// Call after switching style: shows its name briefly and clears any
    /// per-style history.
    pub fn style_changed(&mut self) {
        self.toast = TOAST_SECONDS;
        self.horizon.reset();
    }

    pub fn draw(&mut self, style: Style, frame: &Frame, device: &str) {
        let (w, h) = (screen_width(), screen_height());
        if self.target.texture.width() != w || self.target.texture.height() != h {
            self.target = make_target();
        }

        let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, w, h));
        cam.render_target = Some(self.target.clone());
        set_camera(&cam);

        let ui = Ui { font: self.font.as_ref(), w, h, s: (h / 540.0).clamp(0.8, 2.5) };
        match style {
            Style::Meter => meter::draw(&ui, frame, device),
            Style::Matrix => matrix::draw(&ui, frame, device),
            Style::Mirror => mirror::draw(&ui, frame),
            Style::Chart => chart::draw(&ui, frame, device),
            Style::Horizon => self.horizon.draw(&ui, frame),
        }
        if self.toast > 0.0 {
            draw_toast(&ui, style, (self.toast / 0.3).min(1.0));
            self.toast -= get_frame_time();
        }

        set_default_camera();
        clear_background(BLACK);
        draw_texture_ex(
            &self.target.texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(w, h)),
                // Render targets come out upside down in macroquad.
                flip_y: true,
                ..Default::default()
            },
        );
    }
}

fn make_target() -> RenderTarget {
    let rt = render_target(screen_width().max(1.0) as u32, screen_height().max(1.0) as u32);
    rt.texture.set_filter(FilterMode::Linear);
    rt
}

fn draw_toast(ui: &Ui, style: Style, alpha: f32) {
    let label = style.name().to_uppercase();
    let size = 22.0 * ui.s;
    let (cx, cy) = (ui.w / 2.0, ui.h * 0.14);
    let pad = 14.0 * ui.s;
    let tw = ui.measure(&label, size);
    draw_rectangle(
        cx - tw / 2.0 - pad,
        cy - size * 0.8,
        tw + pad * 2.0,
        size * 1.6,
        with_alpha(BLACK, 0.75 * alpha),
    );
    ui.text_centered(&label, cx, cy, size, with_alpha(TEXT, alpha));
}

/// Per-frame drawing context shared by all styles.
pub(super) struct Ui<'a> {
    font: Option<&'a Font>,
    pub w: f32,
    pub h: f32,
    /// UI scale relative to a 540px-tall window.
    pub s: f32,
}

impl Ui<'_> {
    /// Draw text left-aligned at `x`, vertically centred on `y`.
    pub fn text(&self, s: &str, x: f32, y: f32, size: f32, color: Color) {
        let size = size.round() as u16;
        let dims = measure_text(s, self.font, size, 1.0);
        draw_text_ex(
            s,
            x,
            y - dims.height / 2.0 + dims.offset_y,
            TextParams { font: self.font, font_size: size, color, ..Default::default() },
        );
    }

    pub fn text_centered(&self, s: &str, cx: f32, y: f32, size: f32, color: Color) {
        self.text(s, cx - self.measure(s, size) / 2.0, y, size, color);
    }

    pub fn text_right(&self, s: &str, right: f32, y: f32, size: f32, color: Color) {
        self.text(s, right - self.measure(s, size), y, size, color);
    }

    pub fn measure(&self, s: &str, size: f32) -> f32 {
        measure_text(s, self.font, size.round() as u16, 1.0).width
    }

    /// Clear a gap in a border line so a label of width `w` can sit in it.
    pub fn notch(&self, x: f32, y: f32, w: f32, bg: Color) {
        let gap = 6.0 * self.s;
        draw_rectangle(x - gap, y - 4.0 * self.s, w + gap * 2.0, 8.0 * self.s, bg);
    }

    /// The standard rounded frame with btop-style labels set into its edges.
    /// Returns the frame's rectangle.
    pub fn chrome(&self, style: Style, device: &str, live: bool) -> Rect {
        let pad = 18.0 * self.s;
        let r = Rect::new(pad, pad, self.w - pad * 2.0, self.h - pad * 2.0);
        rounded_rect_lines(r, 10.0 * self.s, 1.5 * self.s, FRAME);

        let size = 15.0 * self.s;
        let small = size * 0.9;
        let inset = 16.0 * self.s;

        // Top-left: app + style name.
        let style_label = format!(" · {}", style.name());
        let name_w = self.measure("vizy", size);
        self.notch(r.x + inset, r.y, name_w + self.measure(&style_label, size), BG);
        self.text("vizy", r.x + inset, r.y, size, ACCENT);
        self.text(&style_label, r.x + inset + name_w, r.y, size, TEXT_DIM);

        // Top-right: live / idle status.
        let (status, color) = if live { ("● LIVE", LIVE) } else { ("○ IDLE", TEXT_DIM) };
        let sw = self.measure(status, size);
        self.notch(r.right() - inset - sw, r.y, sw, BG);
        self.text(status, r.right() - inset - sw, r.y, size, color);

        // Bottom: device on the left, key hints on the right.
        self.notch(r.x + inset, r.bottom(), self.measure(device, small), BG);
        self.text(device, r.x + inset, r.bottom(), small, TEXT_DIM);
        let hw = self.measure(HINT, small);
        self.notch(r.right() - inset - hw, r.bottom(), hw, BG);
        self.text(HINT, r.right() - inset - hw, r.bottom(), small, TEXT_DIM);

        r
    }
}

/// Frequency labels under a spectrum spanning `left..left + width`, kept
/// inside that span so edge labels don't spill over borders.
pub(super) fn freq_axis(ui: &Ui, f: &Frame, left: f32, width: f32, y: f32, size: f32, color: Color) {
    for (hz, label) in FREQ_LABELS {
        if hz < f.freq_lo || hz > f.freq_hi {
            continue;
        }
        let half = ui.measure(label, size) / 2.0;
        let x = (left + f.freq_position(hz) * width).clamp(left + half, left + width - half);
        ui.text_centered(label, x, y, size, color);
    }
}

/// A block of LED dots `cols` wide and `rows` tall showing one level, with
/// the top lit row fading in for smoothness and a brighter peak-hold row.
pub(super) fn led_column(
    level: &Smoothed,
    left: f32,
    bottom: f32,
    spacing: f32,
    cols: usize,
    rows: usize,
) {
    let r = spacing * 0.34;
    let lit = level.value * rows as f32;
    let peak_row = ((level.peak * rows as f32).ceil() as usize).clamp(1, rows) - 1;

    for row in 0..rows {
        let y = bottom - (row as f32 + 0.5) * spacing;
        let base = ramp(&LED_RAMP, row as f32 / (rows - 1) as f32);
        // 0..1 how lit this row is.
        let amount = (lit - row as f32).clamp(0.0, 1.0);
        let is_peak = level.peak > 0.0 && row == peak_row && amount < 1.0;

        for col in 0..cols {
            let x = left + (col as f32 + 0.5) * spacing;
            // Unlit LEDs keep a faint tint of their colour, like real hardware.
            draw_circle(x, y, r, with_alpha(base, 0.07));
            if is_peak {
                led(x, y, r, mix(base, WHITE, 0.35), 1.0);
            } else if amount > 0.0 {
                led(x, y, r, base, amount);
            }
        }
    }
}

/// A lit LED: soft halo layers under a solid core.
pub(super) fn led(x: f32, y: f32, r: f32, color: Color, amount: f32) {
    draw_circle(x, y, r * 2.2, with_alpha(color, 0.05 * amount));
    draw_circle(x, y, r * 1.5, with_alpha(color, 0.12 * amount));
    draw_circle(x, y, r, with_alpha(color, amount));
}

pub(super) fn rounded_rect_lines(r: Rect, radius: f32, t: f32, color: Color) {
    let (x0, y0, x1, y1) = (r.x, r.y, r.right(), r.bottom());
    draw_line(x0 + radius, y0, x1 - radius, y0, t, color);
    draw_line(x0 + radius, y1, x1 - radius, y1, t, color);
    draw_line(x0, y0 + radius, x0, y1 - radius, t, color);
    draw_line(x1, y0 + radius, x1, y1 - radius, t, color);
    // draw_arc grows outward from `radius`, so start half a line width inside.
    let ar = radius - t / 2.0;
    draw_arc(x0 + radius, y0 + radius, 24, ar, 180.0, t, 90.0, color);
    draw_arc(x1 - radius, y0 + radius, 24, ar, 270.0, t, 90.0, color);
    draw_arc(x1 - radius, y1 - radius, 24, ar, 0.0, t, 90.0, color);
    draw_arc(x0 + radius, y1 - radius, 24, ar, 90.0, t, 90.0, color);
}

/// Sample a colour ramp of (position, colour) stops at `t` (0..1).
pub(super) fn ramp(stops: &[(f32, Color)], t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let i = stops.windows(2).position(|w| t <= w[1].0).unwrap_or(stops.len() - 2);
    let (t0, c0) = stops[i];
    let (t1, c1) = stops[i + 1];
    mix(c0, c1, ((t - t0) / (t1 - t0)).clamp(0.0, 1.0))
}

pub(super) fn mix(a: Color, b: Color, k: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * k,
        a.g + (b.g - a.g) * k,
        a.b + (b.b - a.b) * k,
        a.a + (b.a - a.a) * k,
    )
}

pub(super) fn with_alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}
