//! Drawing. The scene only reads the analysis `Frame`; it never touches audio
//! directly.
//!
//! There is one scene, `horizon` (a synthwave drive), seen from one of two
//! cameras: the chase view above the road, or the driver's seat with the
//! dashboard (`dash`). Toggling swoops between them.

mod dash;
mod horizon;

use macroquad::prelude::*;

use crate::analysis::Frame;

/// Spectrum bands per channel the scene wants from the analyzer.
pub const BANDS: usize = 40;
/// Lowest frequency shown, reaching into the sub-bass.
pub const FREQ_LO: f32 = 25.0;

const TEXT_DIM: Color = Color::new(0.42, 0.46, 0.54, 1.0);
const TEXT: Color = Color::new(0.78, 0.82, 0.88, 1.0);
const HINT: &str = "↑/↓ terrain · c cockpit · q quit";
const COCKPIT_HINT: &str = "↑/↓ terrain · ←/→ lane · c chase cam · q quit";

/// Monospace font from the Windows install; falls back to macroquad's built-in one.
const FONT_PATHS: [&str; 2] = [
    "C:\\Windows\\Fonts\\CascadiaMono.ttf",
    "C:\\Windows\\Fonts\\consola.ttf",
];

/// How long the view name shows after switching.
const TOAST_SECONDS: f32 = 1.2;
/// How long the camera takes to swoop between views.
const SWOOP_SECONDS: f32 = 1.4;

pub struct Renderer {
    // The scene is drawn offscreen first so a glow/bloom pass can be added
    // later between the scene and the screen.
    target: RenderTarget,
    font: Option<Font>,
    toast: f32,
    horizon: horizon::Horizon,
    dash: dash::Dash,
    cockpit: bool,
    /// 0 = chase view, 1 = driver's seat; moves toward `cockpit` over the swoop.
    swoop: f32,
}

impl Renderer {
    pub fn new() -> Self {
        let font = FONT_PATHS
            .iter()
            .find_map(|p| std::fs::read(p).ok())
            .and_then(|bytes| load_ttf_font_from_bytes(&bytes).ok());
        Self {
            target: make_target(),
            font,
            toast: TOAST_SECONDS,
            horizon: horizon::Horizon::new(),
            dash: dash::Dash::new(),
            cockpit: false,
            swoop: 0.0,
        }
    }

    /// ↑: surge the mountains.
    pub fn surge(&mut self) {
        self.horizon.surge();
    }

    /// ↓: send a ground wave rolling toward the viewer.
    pub fn send_wave(&mut self) {
        self.horizon.send_wave();
    }

    /// Change lanes; only while driving from the cockpit.
    pub fn change_lane(&mut self, dir: i32) {
        if self.cockpit {
            self.horizon.change_lane(dir);
        }
    }

    /// Swap between the chase view and the driver's seat.
    pub fn toggle_cockpit(&mut self) {
        self.cockpit = !self.cockpit;
        self.toast = TOAST_SECONDS;
    }

    pub fn draw(&mut self, frame: &Frame) {
        let (w, h) = (screen_width(), screen_height());
        if self.target.texture.width() != w || self.target.texture.height() != h {
            self.target = make_target();
        }
        let dt = get_frame_time();
        let goal = if self.cockpit { 1.0 } else { 0.0 };
        let step = dt / SWOOP_SECONDS;
        self.swoop = if self.swoop < goal {
            (self.swoop + step).min(goal)
        } else {
            (self.swoop - step).max(goal)
        };
        // Ease in and out so the camera glides rather than lurches.
        let view = self.swoop * self.swoop * (3.0 - 2.0 * self.swoop);

        let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, w, h));
        cam.render_target = Some(self.target.clone());
        set_camera(&cam);

        let ui = Ui { font: self.font.as_ref(), w, h, s: (h / 540.0).clamp(0.8, 2.5) };
        self.horizon.draw(&ui, frame, view);
        if view > 0.0 {
            let car = dash::Car {
                speed: self.horizon.speed,
                kick: self.horizon.kick,
                jolt: self.horizon.jolt,
                signal: self.horizon.turn_signal(),
            };
            self.dash.draw(&ui, frame, &car, view);
        }

        let s = ui.s;
        let dim = with_alpha(TEXT_DIM, 0.7);
        ui.text("vizy", 16.0 * s, h - 15.0 * s, 13.0 * s, dim);
        let hint = if self.cockpit { COCKPIT_HINT } else { HINT };
        ui.text_right(hint, w - 16.0 * s, h - 15.0 * s, 13.0 * s, dim);
        if self.toast > 0.0 {
            let label = if self.cockpit { "COCKPIT" } else { "CHASE CAM" };
            draw_toast(&ui, label, (self.toast / 0.3).min(1.0));
            self.toast -= dt;
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

fn draw_toast(ui: &Ui, label: &str, alpha: f32) {
    let size = 22.0 * ui.s;
    let (cx, cy) = (ui.w / 2.0, ui.h * 0.14);
    let pad = 14.0 * ui.s;
    let tw = ui.measure(label, size);
    draw_rectangle(
        cx - tw / 2.0 - pad,
        cy - size * 0.8,
        tw + pad * 2.0,
        size * 1.6,
        with_alpha(BLACK, 0.75 * alpha),
    );
    ui.text_centered(label, cx, cy, size, with_alpha(TEXT, alpha));
}

/// Per-frame drawing context.
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
        let size = size.round().max(1.0) as u16;
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
        measure_text(s, self.font, size.round().max(1.0) as u16, 1.0).width
    }
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
