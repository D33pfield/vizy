//! The driver's seat of an 80s talking supercar: windshield frame, rearview
//! mirror, and a dashboard where every display is a visualizer.
//!
//! - Scanner: a red light sweeping across the glare shield, faster when the
//!   music is busier.
//! - Voice box: three red LED columns growing from the centre; left channel,
//!   mids, right channel.
//! - Speedometer: 7-segment MPH readout and bar, showing the drive speed.
//! - Tachometer: bar graph driven by the bass, flashing the redline on kicks.
//! - Stereo: an amber VFD spectrum analyzer with ghosted unlit segments.
//! - Button banks: NORMAL CRUISE / PURSUIT follow loudness, TURBO BOOST
//!   flashes on kicks, the rest blink with the treble.
//! - Turn signal arrows at the top corners of the cluster.
//!
//! Layout is in fractions of the window. During the swoop the dashboard
//! rises from below and the windshield frame closes in from the edges.

use macroquad::prelude::*;

use super::{mix, with_alpha, Ui};
use crate::analysis::Frame;

const BODY: Color = Color::new(0.030, 0.018, 0.045, 1.0);
const PANEL: Color = Color::new(0.012, 0.008, 0.020, 1.0);
const TRIM: Color = Color::new(1.0, 0.25, 0.55, 1.0);
const RED: Color = Color::new(1.0, 0.12, 0.10, 1.0);
const AMBER: Color = Color::new(1.0, 0.62, 0.12, 1.0);
const GREEN: Color = Color::new(0.35, 1.0, 0.45, 1.0);
const CYAN: Color = Color::new(0.30, 0.95, 1.0, 1.0);
/// Brightness of unlit segments, like a real VFD or LED panel.
const GHOST: f32 = 0.09;

const SCANNER_CELLS: usize = 16;
/// Displayed MPH per unit of drive speed (cruise ≈ 15, loud ≈ 90).
const MPH_PER_SPEED: f32 = 30.0;
const MPH_MAX: f32 = 120.0;
/// How quickly the MPH readout follows the drive speed (per second); low,
/// so it shows a running average rather than every kick.
const MPH_RATE: f32 = 0.6;
/// PURSUIT lights above this loudness and goes back to NORMAL CRUISE below
/// the lower one; the gap stops it flickering at the boundary.
const PURSUIT_ON: f32 = 0.5;
const PURSUIT_OFF: f32 = 0.35;
/// Toggles per second for each activity light: a slow base, more with treble.
const ACTIVITY_BASE: f32 = 0.05;
const ACTIVITY_TREBLE: f32 = 0.3;
const ANALYZER_BANDS: usize = 16;
const ANALYZER_SEGS: usize = 10;
const TACH_SEGS: usize = 24;
/// Tach segments at the top end drawn as the redline.
const REDLINE_SEGS: usize = 5;

/// 7-segment patterns for 0-9: bit 0 = top, then clockwise, bit 6 = middle.
const SEG7: [u8; 10] = [0x3F, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F];

const LEFT_BUTTONS: [&str; 5] = ["AUTO CRUISE", "NORMAL CRUISE", "PURSUIT", "TURBO BOOST", "SKI MODE"];
const RIGHT_BUTTONS: [&str; 5] = ["AUDIO SCAN", "SURVEILLANCE", "INFRA RED", "TRACTION", "EJECT"];

/// What the dash needs to know about the car this frame.
pub(super) struct Car {
    /// Drive speed, in depth units per second.
    pub speed: f32,
    /// Kick-drum envelope, 0..1.
    pub kick: f32,
    /// Lit turn signal: -1 left, 1 right, 0 none.
    pub signal: i32,
    /// Bump from a ground wave reaching the car, 0..1.
    pub jolt: f32,
}

pub struct Dash {
    scanner_phase: f32,
    /// Brightness of each scanner cell, so the light leaves a fading trail.
    scanner: [f32; SCANNER_CELLS],
    /// Displayed speed, eased so the digits don't jitter.
    mph: f32,
    /// On/off state of the buttons that blink with the treble.
    activity: [bool; 6],
    pursuit: bool,
    rng: u32,
}

/// Coordinates for one frame: fractions of the window, with the dashboard
/// shifted down by `dy` while it slides in.
struct Layout {
    w: f32,
    h: f32,
    dy: f32,
}

impl Layout {
    fn x(&self, fx: f32) -> f32 {
        fx * self.w
    }
    fn y(&self, fy: f32) -> f32 {
        fy * self.h + self.dy
    }
    fn p(&self, fx: f32, fy: f32) -> Vec2 {
        vec2(self.x(fx), self.y(fy))
    }
    fn rect(&self, fx: f32, fy: f32, fw: f32, fh: f32) -> Rect {
        Rect::new(self.x(fx), self.y(fy), fw * self.w, fh * self.h)
    }
}

impl Dash {
    pub fn new() -> Self {
        Self {
            scanner_phase: 0.0,
            scanner: [0.0; SCANNER_CELLS],
            mph: 0.0,
            activity: [false; 6],
            pursuit: false,
            rng: 0x1234_5679,
        }
    }

    /// `view` is 0 in the chase view and 1 fully in the driver's seat.
    pub(super) fn draw(&mut self, ui: &Ui, f: &Frame, car: &Car, view: f32) {
        let (speed, kick) = (car.speed, car.kick);
        let dt = get_frame_time();
        let n = f.mono.len();
        let energy = avg(f, 0..n);
        let bass = avg(f, 0..n / 4);
        let mids = avg(f, n / 4..3 * n / 4);
        let treble = avg(f, 3 * n / 4..n);
        self.update(dt, speed, energy, treble);

        // The dash rises from below and dips a little on kicks, like the
        // suspension taking a bump.
        let away = 1.0 - view;
        let bump = kick * 0.008 + car.jolt * 0.025;
        let dash = Layout { w: ui.w, h: ui.h, dy: away * 0.75 * ui.h + bump * ui.h };

        draw_body(&dash);
        draw_turn_signals(&dash, car.signal);
        self.draw_scanner(&dash);
        draw_voice_box(&dash, f.left.level.value, mids, f.right.level.value);
        self.draw_speedo(ui, &dash);
        draw_tach(ui, &dash, (bass * 0.85 + kick * 0.3).min(1.0), kick);
        draw_analyzer(ui, &dash, f);
        self.draw_buttons(ui, &dash, kick);
        draw_yoke(&dash);
        draw_frame(ui, away);
    }

    fn update(&mut self, dt: f32, speed: f32, energy: f32, treble: f32) {
        self.mph += (speed * MPH_PER_SPEED - self.mph) * (dt * MPH_RATE).min(1.0);
        if energy > PURSUIT_ON {
            self.pursuit = true;
        } else if energy < PURSUIT_OFF {
            self.pursuit = false;
        }

        self.scanner_phase += dt * (2.0 + 4.0 * energy);
        let pos = (self.scanner_phase.sin() * 0.5 + 0.5) * (SCANNER_CELLS - 1) as f32;
        for (i, cell) in self.scanner.iter_mut().enumerate() {
            *cell *= (-dt * 5.0).exp();
            if i == pos.round() as usize {
                *cell = 1.0;
            }
        }

        // Random flicker, busier with more treble, like a computer thinking.
        for i in 0..self.activity.len() {
            if self.random() < dt * (ACTIVITY_BASE + ACTIVITY_TREBLE * treble) {
                self.activity[i] = !self.activity[i];
            }
        }
    }

    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as f32 / u32::MAX as f32
    }

    /// Red light sweeping along the glare shield.
    fn draw_scanner(&self, l: &Layout) {
        let (x0, width) = (0.32, 0.36);
        let cw = width / SCANNER_CELLS as f32;
        for (i, &v) in self.scanner.iter().enumerate() {
            let r = l.rect(x0 + i as f32 * cw + cw * 0.1, 0.594, cw * 0.8, 0.012);
            lamp(r, RED, v);
        }
    }

    /// Left of the voice box: 7-segment MPH and a speed bar.
    fn draw_speedo(&self, ui: &Ui, l: &Layout) {
        let color = mix(RED, AMBER, 0.45);
        let mph = self.mph.round().clamp(0.0, 999.0) as u32;
        let digits = [mph / 100, mph / 10 % 10, mph % 10];
        for (i, &d) in digits.iter().enumerate() {
            // Leading zeros stay dark, like a real readout.
            let lit = if (i == 0 && mph < 100) || (i == 1 && mph < 10) { 0 } else { SEG7[d as usize] };
            seven_segment(l.rect(0.245 + i as f32 * 0.047, 0.66, 0.036, 0.095), lit, color);
        }
        ui.text("MPH", l.x(0.392), l.y(0.745), 11.0 * ui.s, with_alpha(color, 0.9));

        let fill = (self.mph / MPH_MAX).clamp(0.0, 1.0);
        bar_graph(l, 0.235, 0.79, 0.195, 0.022, 24, fill, |t| {
            if t < 0.6 { GREEN } else if t < 0.85 { AMBER } else { RED }
        });
        for (t, label) in [(0.0, "0"), (0.5, "60"), (1.0, "120")] {
            let x = l.x(0.235 + t * 0.195);
            ui.text_centered(label, x, l.y(0.83), 9.0 * ui.s, with_alpha(color, 0.6));
        }
    }

    fn draw_buttons(&self, ui: &Ui, l: &Layout, kick: f32) {
        let pursuit = self.pursuit;
        let left = [
            (GREEN, 1.0),
            (GREEN, if pursuit { 0.0 } else { 1.0 }),
            (AMBER, if pursuit { 1.0 } else { 0.0 }),
            (RED, if kick > 0.7 { 1.0 } else { 0.0 }),
            (CYAN, on(self.activity[0])),
        ];
        let right = [
            (CYAN, 1.0),
            (AMBER, on(self.activity[1])),
            (RED, on(self.activity[2])),
            (GREEN, on(self.activity[3])),
            (AMBER, on(self.activity[4] && self.activity[5])),
        ];
        for (x, labels, states) in [(0.035, LEFT_BUTTONS, left), (0.815, RIGHT_BUTTONS, right)] {
            for (i, (label, (color, lit))) in labels.iter().zip(states).enumerate() {
                button(ui, l.rect(x, 0.665 + i as f32 * 0.042, 0.15, 0.032), label, color, lit);
            }
        }
    }
}

fn on(b: bool) -> f32 {
    if b { 1.0 } else { 0.0 }
}

fn avg(f: &Frame, r: std::ops::Range<usize>) -> f32 {
    let s = &f.mono[r];
    s.iter().map(|b| b.value).sum::<f32>() / s.len().max(1) as f32
}

/// Dashboard body, glare shield over the instrument cluster, neon trim.
fn draw_body(l: &Layout) {
    draw_rectangle(0.0, l.y(0.62), l.w, l.h * 0.5, BODY);
    let shield = [l.p(0.17, 0.66), l.p(0.24, 0.585), l.p(0.76, 0.585), l.p(0.83, 0.66)];
    draw_triangle(shield[0], shield[1], shield[2], BODY);
    draw_triangle(shield[0], shield[2], shield[3], BODY);
    for w in shield.windows(2) {
        glow_line(w[0], w[1], 2.0, TRIM, 0.8);
    }
    glow_line(l.p(0.0, 0.62), l.p(0.185, 0.62), 2.0, TRIM, 0.6);
    glow_line(l.p(0.815, 0.62), l.p(1.0, 0.62), 2.0, TRIM, 0.6);

    let panel = l.rect(0.21, 0.635, 0.58, 0.23);
    draw_rectangle(panel.x, panel.y, panel.w, panel.h, PANEL);
    draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 1.5, with_alpha(TRIM, 0.35));
}

/// Green turn signal arrows in the top corners of the cluster.
fn draw_turn_signals(l: &Layout, lit: i32) {
    for dir in [-1, 1] {
        let cx = if dir < 0 { 0.232 } else { 0.768 };
        let d = dir as f32;
        let (tip, back) = (l.p(cx + d * 0.012, 0.655), cx - d * 0.004);
        let (top, bottom) = (l.p(back, 0.643), l.p(back, 0.667));
        let amount = if lit == dir { 1.0 } else { 0.0 };
        // Ghost arrow always, glowing arrow when lit.
        draw_triangle(tip, top, bottom, with_alpha(GREEN, GHOST));
        lamp(l.rect(back.min(cx - d * 0.012), 0.651, 0.008, 0.008), GREEN, amount);
        if amount > 0.0 {
            let grow = |p: Vec2| tip + (p - tip) * 1.5;
            draw_triangle(tip + vec2(d * 3.0, 0.0), grow(top), grow(bottom), with_alpha(GREEN, 0.15));
            draw_triangle(tip, top, bottom, GREEN);
        }
    }
}

/// Three LED columns growing up and down from a centre line.
fn draw_voice_box(l: &Layout, left: f32, mid: f32, right: f32) {
    let (cy, half_h) = (0.75, 0.095);
    for (fx, level, segs) in [(0.465, left, 6), (0.5, mid, 8), (0.535, right, 6)] {
        let pitch = half_h / 8.0;
        let lit = level * segs as f32;
        for i in 0..segs {
            let amount = (lit - i as f32).clamp(0.0, 1.0);
            for dir in [-1.0, 1.0] {
                let fy = cy + dir * (i as f32 + 0.5) * pitch - pitch * 0.35;
                lamp(l.rect(fx - 0.011, fy, 0.022, pitch * 0.7), RED, amount);
            }
        }
    }
}

/// Bar-graph tachometer: segments grow taller toward the redline.
fn draw_tach(ui: &Ui, l: &Layout, rpm: f32, kick: f32) {
    let (x0, width, base, max_h) = (0.575, 0.2, 0.705, 0.04);
    let sw = width / TACH_SEGS as f32;
    let lit = rpm * TACH_SEGS as f32;
    let flash = kick > 0.6 && (get_time() * 20.0) as i64 % 2 == 0;
    for i in 0..TACH_SEGS {
        let t = i as f32 / (TACH_SEGS - 1) as f32;
        let hgt = max_h * (0.35 + 0.65 * t);
        let red = i >= TACH_SEGS - REDLINE_SEGS;
        let color = if red { RED } else if t > 0.6 { AMBER } else { GREEN };
        let amount = if red && flash { 1.0 } else { (lit - i as f32).clamp(0.0, 1.0) };
        lamp(l.rect(x0 + i as f32 * sw + sw * 0.15, base - hgt, sw * 0.7, hgt), color, amount);
    }
    ui.text("RPM x1000", l.x(x0), l.y(0.65), 9.0 * ui.s, with_alpha(AMBER, 0.7));
}

/// The stereo: amber VFD spectrum analyzer with peak segments.
fn draw_analyzer(ui: &Ui, l: &Layout, f: &Frame) {
    let (x0, width, top, height) = (0.575, 0.2, 0.725, 0.105);
    let bw = width / ANALYZER_BANDS as f32;
    let pitch = height / ANALYZER_SEGS as f32;
    let chunk = f.mono.len().div_ceil(ANALYZER_BANDS);
    for (b, bands) in f.mono.chunks(chunk).enumerate() {
        let value = bands.iter().map(|s| s.value).fold(0.0, f32::max);
        let peak = bands.iter().map(|s| s.peak).fold(0.0, f32::max);
        let lit = value * ANALYZER_SEGS as f32;
        let peak_seg = ((peak * ANALYZER_SEGS as f32).ceil() as usize).clamp(1, ANALYZER_SEGS) - 1;
        for i in 0..ANALYZER_SEGS {
            let amount = (lit - i as f32).clamp(0.0, 1.0);
            let amount = if i == peak_seg && peak > 0.0 { amount.max(0.8) } else { amount };
            let fy = top + height - (i + 1) as f32 * pitch;
            lamp(l.rect(x0 + b as f32 * bw + bw * 0.12, fy, bw * 0.76, pitch * 0.55), AMBER, amount);
        }
    }
    let label = with_alpha(CYAN, 0.7);
    for (t, text) in [(0.0, "30"), (0.33, "250"), (0.66, "2K"), (1.0, "16K")] {
        let x = l.x(x0 + t * (width - bw) + bw / 2.0);
        ui.text_centered(text, x, l.y(top + height + 0.017), 8.0 * ui.s, label);
    }
}

/// Gullwing steering yoke at the bottom of the view.
fn draw_yoke(l: &Layout) {
    let bar = (l.p(0.38, 0.885), l.p(0.62, 0.885));
    let grips = [(l.p(0.38, 0.885), l.p(0.355, 1.03)), (l.p(0.62, 0.885), l.p(0.645, 1.03))];
    let (bar_t, grip_t) = (0.03 * l.h, 0.05 * l.h);
    // Trim first, then the body slightly thinner on top, for a neon outline.
    for (color, extra) in [(with_alpha(TRIM, 0.7), 3.0), (BODY, 0.0)] {
        draw_line(bar.0.x, bar.0.y, bar.1.x, bar.1.y, bar_t + extra, color);
        for (a, b) in grips {
            draw_line(a.x, a.y, b.x, b.y, grip_t + extra, color);
        }
        let hub = l.rect(0.455, 0.875, 0.09, 0.065);
        draw_rectangle(hub.x - extra / 2.0, hub.y - extra / 2.0, hub.w + extra, hub.h + extra, color);
    }
    let led = l.rect(0.493, 0.9, 0.014, 0.012);
    lamp(led, RED, 0.6 + 0.4 * (get_time() as f32 * 3.0).sin().abs());
}

/// Roof edge, A-pillars and rearview mirror; they close in from the edges
/// as `away` goes from 1 to 0.
fn draw_frame(ui: &Ui, away: f32) {
    let (w, h) = (ui.w, ui.h);
    let ty = -away * 0.3 * h;
    let tx = away * 0.2 * w;

    draw_rectangle(0.0, ty, w, 0.04 * h, BODY);
    glow_line(vec2(0.0, ty + 0.04 * h), vec2(w, ty + 0.04 * h), 1.5, TRIM, 0.5);

    for side in [-1.0f32, 1.0] {
        // Mirror x around the centre for the right pillar.
        let px = |fx: f32| if side < 0.0 { fx * w - tx } else { w - fx * w + tx };
        let pts = [vec2(px(0.0), 0.0), vec2(px(0.12), 0.0), vec2(px(0.03), 0.64 * h), vec2(px(0.0), 0.64 * h)];
        draw_triangle(pts[0], pts[1], pts[2], BODY);
        draw_triangle(pts[0], pts[2], pts[3], BODY);
        glow_line(pts[1], pts[2], 2.0, TRIM, 0.6);
    }

    // Rearview mirror, showing the road falling away behind.
    let g = Rect::new(0.42 * w, 0.055 * h + ty, 0.16 * w, 0.055 * h);
    draw_line(w / 2.0, ty + 0.04 * h, w / 2.0, g.y, 4.0, BODY);
    draw_rectangle(g.x - 4.0, g.y - 4.0, g.w + 8.0, g.h + 8.0, BODY);
    draw_rectangle(g.x, g.y, g.w, g.h / 2.0, Color::new(0.10, 0.04, 0.18, 1.0));
    draw_rectangle(g.x, g.y + g.h / 2.0, g.w, g.h / 2.0, Color::new(0.03, 0.01, 0.06, 1.0));
    let vanish = vec2(g.center().x, g.y + g.h / 2.0);
    let (bl, br) = (vec2(g.x + g.w * 0.3, g.bottom()), vec2(g.x + g.w * 0.7, g.bottom()));
    draw_triangle(bl, br, vanish, Color::new(0.06, 0.03, 0.10, 1.0));
    glow_line(bl, vanish, 1.0, TRIM, 0.8);
    glow_line(br, vanish, 1.0, TRIM, 0.8);
    glow_line(vec2(g.x, vanish.y), vec2(g.right(), vanish.y), 1.0, TRIM, 0.6);
    draw_rectangle_lines(g.x - 4.0, g.y - 4.0, g.w + 8.0, g.h + 8.0, 1.5, with_alpha(TRIM, 0.5));
}

/// A labelled dash button. Lit buttons glow softly (backlit lettering and
/// a tinted face) rather than turning into solid blocks of colour.
fn button(ui: &Ui, r: Rect, label: &str, color: Color, lit: f32) {
    draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
    if lit > 0.0 {
        draw_rectangle(r.x, r.y, r.w, r.h, with_alpha(color, 0.16 * lit));
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, with_alpha(color, 0.6 * lit));
        ui.text_centered(label, r.center().x, r.center().y, 9.0 * ui.s, with_alpha(color, lit));
    } else {
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, with_alpha(color, 0.15));
        ui.text_centered(label, r.center().x, r.center().y, 9.0 * ui.s, with_alpha(color, 0.22));
    }
}

/// A row of `segs` segments filling left to right up to `fill` (0..1).
fn bar_graph(l: &Layout, fx: f32, fy: f32, fw: f32, fh: f32, segs: usize, fill: f32, color: impl Fn(f32) -> Color) {
    let sw = fw / segs as f32;
    let lit = fill * segs as f32;
    for i in 0..segs {
        let t = i as f32 / (segs - 1) as f32;
        let amount = (lit - i as f32).clamp(0.0, 1.0);
        lamp(l.rect(fx + i as f32 * sw + sw * 0.15, fy, sw * 0.7, fh), color(t), amount);
    }
}

/// One 7-segment digit in `r`; `bits` says which segments are lit.
fn seven_segment(r: Rect, bits: u8, color: Color) {
    let t = r.w * 0.18;
    let (x, y, w, h) = (r.x, r.y, r.w, r.h);
    let half = h / 2.0;
    let segs = [
        Rect::new(x + t, y, w - 2.0 * t, t),                        // top
        Rect::new(x + w - t, y + t, t, half - 1.5 * t),             // upper right
        Rect::new(x + w - t, y + half + 0.5 * t, t, half - 1.5 * t), // lower right
        Rect::new(x + t, y + h - t, w - 2.0 * t, t),                // bottom
        Rect::new(x, y + half + 0.5 * t, t, half - 1.5 * t),        // lower left
        Rect::new(x, y + t, t, half - 1.5 * t),                     // upper left
        Rect::new(x + t, y + half - 0.5 * t, w - 2.0 * t, t),       // middle
    ];
    for (i, seg) in segs.iter().enumerate() {
        lamp(*seg, color, if bits & (1 << i) != 0 { 1.0 } else { 0.0 });
    }
}

/// A display segment: always faintly visible, glowing when lit.
fn lamp(r: Rect, color: Color, amount: f32) {
    draw_rectangle(r.x, r.y, r.w, r.h, with_alpha(color, GHOST));
    if amount > 0.0 {
        let g = r.h.min(r.w) * 0.6;
        draw_rectangle(r.x - g, r.y - g, r.w + g * 2.0, r.h + g * 2.0, with_alpha(color, 0.12 * amount));
        draw_rectangle(r.x, r.y, r.w, r.h, with_alpha(color, amount));
    }
}

/// A line with a soft wider halo under it.
fn glow_line(a: Vec2, b: Vec2, thickness: f32, color: Color, alpha: f32) {
    draw_line(a.x, a.y, b.x, b.y, thickness * 3.0, with_alpha(color, 0.12 * alpha));
    draw_line(a.x, a.y, b.x, b.y, thickness, with_alpha(color, alpha));
}
