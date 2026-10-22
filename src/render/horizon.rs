//! horizon: an 80s synthwave drive. A road runs over a neon grid toward a
//! striped sun, palm trees slide past on both sides, and a wireframe mountain
//! range on the horizon is shaped by the spectrum.
//!
//! The ground curves down like a small planet: the horizon is the crest of
//! that curve, the road bends over it and vanishes, and the mountain range
//! sits beyond it, sinking further the further back it goes.
//!
//! - Mountains: left channel forms the left half and right channel the right
//!   half, bass at the outer edges and treble meeting in a pass in the middle
//!   where the road and sun are. Each row further back shows the spectrum a
//!   little later, so the music ripples back through the range.
//! - Movement: the road, grid and trees come toward the viewer; speed follows
//!   the overall loudness, with a surge on kicks.
//! - Palms: silhouettes, each grown from the music at the moment it appears
//!   on the horizon (bass droops the fronds, treble adds fronds and fuller
//!   leaves, mids make it taller and wider), so the passing rows are a record
//!   of the song.
//! - Kicks pulse the sun and flash the nearest mountain ridges; busy treble
//!   makes the stars twinkle.

use std::collections::{HashMap, VecDeque};
use std::f32::consts::{PI, TAU};

use macroquad::prelude::*;

use super::{mix, ramp, with_alpha, Ui, HINT, TEXT_DIM};
use crate::analysis::Frame;

// Ground shape. Depth z = 1 is near the bottom of the window; the ground
// curves down so it peaks on screen (the visible horizon) at CREST_Z.
/// Screen y (fraction of height) of the ground at depth 1.
const BOTTOM_Y: f32 = 0.97;
/// Screen y of the crest, i.e. the visible horizon.
const CREST_Y: f32 = 0.5;
const CREST_Z: f32 = 4.5;
/// Nearest depth drawn (just below the bottom of the window).
const NEAR_Z: f32 = 0.9;

/// Mountains run from the crest back to MTN_FAR_Z in MTN_ROWS rows.
const MTN_FAR_Z: f32 = 13.0;
const MTN_ROWS: usize = 22;
/// How fast the spectrum ripples back through the mountain rows.
const MTN_ROWS_PER_SECOND: f32 = 10.0;
/// Tallest peak, as a fraction of window height, at the front of the range.
const MTN_AMP: f32 = 0.45;
/// >1 sharpens peaks: loud bands stand out more against quiet neighbours.
const MTN_SHARPNESS: f32 = 1.4;
/// Width of the front mountain row as a fraction of the window. Under 1 so
/// the bass at both ends stays on screen.
const MTN_SPAN: f32 = 0.96;
/// Extra columns at each end that slope the range down to the ground.
const MTN_PAD: usize = 2;
/// Edge squeeze of the mountain columns: quiet music gives even columns,
/// loud music squeezes the ends (bass and treble) and widens the middle.
const SQUEEZE_QUIET: f32 = 0.05;
const SQUEEZE_LOUD: f32 = 1.8;
/// How fast the squeeze follows the music (per second).
const SQUEEZE_RATE: f32 = 2.5;

/// Ground units: 1.0 is one window width at depth 1.
const ROAD_HALF: f32 = 0.16;
const GRID_DX: f32 = 0.12;
const GRID_DZ: f32 = 0.35;
const TREE_X: f32 = 0.30;
const TREE_GAP: f32 = 0.45;
/// Palm height at depth 1, as a fraction of window height.
const TREE_H: f32 = 0.3;
const TREE_NEAR_Z: f32 = 0.25;
/// Palms take on the haze colour from this depth to the crest.
const TREE_FOG_FROM: f32 = 1.5;

/// Travel speed in depth units per second.
const CRUISE_SPEED: f32 = 0.5;
const MUSIC_SPEED: f32 = 1.6;
const KICK_SPEED: f32 = 1.2;
const STARS: usize = 90;

const SKY_TOP: Color = Color::new(0.03, 0.02, 0.10, 1.0);
const SKY_LOW: Color = Color::new(0.42, 0.12, 0.42, 1.0);
const HAZE: Color = Color::new(0.90, 0.40, 0.80, 1.0);
const FLOOR_FAR: Color = Color::new(0.20, 0.05, 0.28, 1.0);
const FLOOR_NEAR: Color = Color::new(0.03, 0.01, 0.06, 1.0);
const GRID: Color = Color::new(1.0, 0.25, 0.80, 1.0);
const ROAD: Color = Color::new(0.03, 0.02, 0.07, 0.92);
const LANE: Color = Color::new(0.95, 0.95, 1.0, 1.0);
const CENTER_LINE: Color = Color::new(1.0, 0.80, 0.20, 1.0);
const MTN_LINE: Color = Color::new(0.30, 0.95, 1.0, 1.0);
const MTN_FILL_NEAR: Color = Color::new(0.10, 0.04, 0.25, 1.0);
const MTN_FILL_FAR: Color = Color::new(0.22, 0.08, 0.34, 1.0);
const SILHOUETTE: Color = Color::new(0.035, 0.01, 0.07, 1.0);
const TREE_FOG: Color = Color::new(0.38, 0.13, 0.42, 1.0);
const SUN: [(f32, Color); 3] = [
    (0.0, Color::new(1.0, 0.92, 0.35, 1.0)),
    (0.55, Color::new(1.0, 0.45, 0.35, 1.0)),
    (1.0, Color::new(0.95, 0.15, 0.55, 1.0)),
];

pub struct Horizon {
    /// Mountain profiles, newest first; hist[0] is always the live spectrum.
    hist: VecDeque<Vec<f32>>,
    /// Fraction of the way to the next mountain row push (0..1).
    clock: f32,
    /// Total distance driven, in depth units.
    travel: f32,
    time: f32,
    bass_slow: f32,
    /// Kick-drum envelope, 0..1.
    kick: f32,
    /// Current mountain edge squeeze, between SQUEEZE_QUIET and SQUEEZE_LOUD.
    squeeze: f32,
    /// (x, y, twinkle phase), x and y in 0..1.
    stars: Vec<(f32, f32, f32)>,
    /// Shape of each palm on screen, keyed by (side, position along the road).
    palms: HashMap<(usize, i64), Palm>,
}

/// One palm's shape, decided by the music when it first appears.
#[derive(Clone, Copy)]
struct Palm {
    height: f32,
    /// Trunk lean; positive leans away from the road.
    lean: f32,
    fronds: usize,
    frond_len: f32,
    droop: f32,
    /// Degrees covered by the fan of fronds.
    spread: f32,
    /// Leaflet length: how full the fronds look.
    leaf: f32,
    phase: f32,
}

impl Palm {
    fn grow(f: &Frame, seed: u32) -> Self {
        let mut rnd = xorshift(seed.wrapping_mul(0x9e37_79b9) | 1);
        rnd();
        let n = f.mono.len();
        let avg = |r: std::ops::Range<usize>| {
            let s = &f.mono[r];
            s.iter().map(|b| b.value).sum::<f32>() / s.len().max(1) as f32
        };
        let (bass, mid, treble) = (avg(0..n / 4), avg(n / 4..3 * n / 4), avg(3 * n / 4..n));
        Self {
            height: 0.8 + 0.35 * mid + 0.15 * rnd(),
            lean: (0.06 + 0.16 * rnd()) * if rnd() < 0.2 { -1.0 } else { 1.0 },
            fronds: 5 + (treble * 6.0).round() as usize + (rnd() * 2.0) as usize,
            frond_len: 0.42 + 0.25 * bass + 0.1 * rnd(),
            droop: 0.15 + 0.6 * bass + 0.15 * rnd(),
            spread: 120.0 + 50.0 * mid + 15.0 * rnd(),
            leaf: 0.09 + 0.10 * treble + 0.03 * rnd(),
            phase: rnd() * TAU,
        }
    }
}

/// Projection for the curved ground: y = h0 + g / z + c * z. The g / z term
/// is ordinary perspective; c * z bends the ground down with distance, which
/// puts the highest on-screen point (the crest) at z = sqrt(g / c) = CREST_Z.
struct Cam {
    cx: f32,
    w: f32,
    h0: f32,
    g: f32,
    c: f32,
    crest_y: f32,
}

impl Cam {
    fn new(ui: &Ui) -> Self {
        let (b, crest) = (BOTTOM_Y * ui.h, CREST_Y * ui.h);
        let c = (b - crest) / ((CREST_Z - 1.0) * (CREST_Z - 1.0));
        Self {
            cx: ui.w / 2.0,
            w: ui.w,
            h0: crest - 2.0 * c * CREST_Z,
            g: c * CREST_Z * CREST_Z,
            c,
            crest_y: crest,
        }
    }

    fn y(&self, z: f32) -> f32 {
        self.h0 + self.g / z + self.c * z
    }

    fn ground(&self, x: f32, z: f32) -> Vec2 {
        vec2(self.cx + x * self.w / z, self.y(z))
    }

    /// Points along a ground line at offset `x`, from z0 to z1 (≤ the crest),
    /// spaced so the curve stays smooth near the crest.
    fn ground_line(&self, x: f32, z0: f32, z1: f32) -> Vec<Vec2> {
        let steps = 16;
        (0..=steps)
            .map(|i| self.ground(x, z0 * (z1 / z0).powf(i as f32 / steps as f32)))
            .collect()
    }
}

impl Horizon {
    pub fn new() -> Self {
        let mut rnd = xorshift(0x2545_f491);
        let stars = (0..STARS).map(|_| (rnd(), rnd(), rnd() * TAU)).collect();
        Self {
            hist: VecDeque::new(),
            clock: 0.0,
            travel: 0.0,
            time: 0.0,
            bass_slow: 0.0,
            kick: 0.0,
            squeeze: SQUEEZE_QUIET,
            stars,
            palms: HashMap::new(),
        }
    }

    /// Forget old mountain rows, so switching back doesn't replay stale history.
    pub fn reset(&mut self) {
        self.hist.clear();
        self.clock = 0.0;
    }

    pub(super) fn draw(&mut self, ui: &Ui, f: &Frame) {
        let dt = get_frame_time();
        self.time += dt;
        self.update_kick(f, dt);
        self.update_mountains(f, dt);
        let energy = f.mono.iter().map(|b| b.value).sum::<f32>() / f.mono.len().max(1) as f32;
        self.travel += dt * (CRUISE_SPEED + MUSIC_SPEED * energy + KICK_SPEED * self.kick);
        let squeeze = SQUEEZE_QUIET + (SQUEEZE_LOUD - SQUEEZE_QUIET) * smoothstep(0.1, 0.7, energy);
        self.squeeze += (squeeze - self.squeeze) * (dt * SQUEEZE_RATE).min(1.0);

        let cam = Cam::new(ui);
        self.draw_sky(ui, f, cam.crest_y);
        self.draw_sun(cam.cx, cam.crest_y, ui.h * 0.2 * (1.0 + 0.08 * self.kick));
        self.draw_mountains(ui, &cam);
        // The ground covers the mountains' bases below the crest.
        self.draw_ground(ui, &cam);
        draw_haze(ui, cam.crest_y);
        self.draw_palms(ui, &cam, f);

        let s = ui.s;
        let y = ui.h - 15.0 * s;
        let dim = with_alpha(TEXT_DIM, 0.7);
        ui.text("vizy · horizon   L ◀ ▶ R", 16.0 * s, y, 13.0 * s, dim);
        ui.text_right(HINT, ui.w - 16.0 * s, y, 13.0 * s, dim);
    }

    /// A kick is the bass jumping above its recent average.
    fn update_kick(&mut self, f: &Frame, dt: f32) {
        let low = &f.mono[..f.mono.len().min(4)];
        let bass = low.iter().map(|b| b.value).sum::<f32>() / low.len().max(1) as f32;
        let jump = (bass - self.bass_slow).max(0.0);
        self.bass_slow += (bass - self.bass_slow) * (dt * 3.0).min(1.0);
        self.kick = (self.kick * (-dt * 6.0).exp()).max((jump * 3.0).min(1.0));
    }

    fn update_mountains(&mut self, f: &Frame, dt: f32) {
        // Left channel low→high, then right high→low: bass at both edges.
        let raw: Vec<f32> =
            f.left.bands.iter().chain(f.right.bands.iter().rev()).map(|b| b.value).collect();
        // Half as many points as bands, for chunky wireframe cells.
        let coarse: Vec<f32> = raw.chunks(2).map(|c| c.iter().sum::<f32>() / c.len() as f32).collect();
        let n = coarse.len();
        let shaped = coarse
            .iter()
            .enumerate()
            .map(|(i, &v)| v.powf(MTN_SHARPNESS) * crag(i) * pass(i as f32 / (n - 1) as f32));
        // Pad both ends with a slope down to zero, so the outermost bass
        // bands are full peaks rather than a cliff at the edge.
        let (first, last) = (coarse[0].powf(MTN_SHARPNESS), coarse[n - 1].powf(MTN_SHARPNESS));
        let slope = |edge: f32, k: usize| edge * k as f32 / (MTN_PAD + 1) as f32;
        let profile: Vec<f32> = (0..MTN_PAD)
            .map(|k| slope(first, k))
            .chain(shaped)
            .chain((0..MTN_PAD).rev().map(|k| slope(last, k)))
            .collect();
        let n = profile.len();

        // History from another band count (after a style switch) can't be reused.
        if self.hist.front().is_none_or(|r| r.len() != n) {
            self.hist = std::iter::repeat_n(vec![0.0; n], MTN_ROWS + 1).collect();
        }
        self.clock += dt * MTN_ROWS_PER_SECOND;
        while self.clock >= 1.0 {
            self.clock -= 1.0;
            self.hist.push_front(profile.clone());
            self.hist.truncate(MTN_ROWS + 1);
        }
        self.hist[0] = profile;
    }

    fn draw_sky(&self, ui: &Ui, f: &Frame, horizon: f32) {
        gradient_band(0.0, horizon, ui.w, SKY_TOP, SKY_LOW, 2.0);

        // Busy treble makes the stars twinkle harder.
        let high = &f.mono[f.mono.len().saturating_sub(8)..];
        let treble = high.iter().map(|b| b.value).sum::<f32>() / high.len().max(1) as f32;
        for &(x, y, phase) in &self.stars {
            let twinkle = 0.5 + 0.5 * (self.time * (1.5 + phase) + phase).sin();
            let a = (0.15 + 0.45 * twinkle) * (0.6 + 0.9 * treble);
            draw_circle(x * ui.w, y * horizon * 0.8, 1.2 * ui.s, with_alpha(WHITE, a.min(1.0)));
        }
    }

    /// Sun sinking into the horizon, with gaps in its lower half drifting down.
    fn draw_sun(&self, cx: f32, horizon: f32, r: f32) {
        let cy = horizon - r * 0.3;
        draw_poly(cx, cy, 96, r * 1.7, 0.0, with_alpha(SUN[2].1, 0.05));
        draw_poly(cx, cy, 96, r * 1.3, 0.0, with_alpha(SUN[1].1, 0.08));

        let step = 2.0;
        let mut y = cy - r;
        while y < horizon {
            let dy = y - cy;
            let half = (r * r - dy * dy).max(0.0).sqrt();
            let t = (y - (cy - r)) / (2.0 * r);
            let stripe = ((y - cy) / (r * 0.14) - self.time * 0.6).rem_euclid(1.0);
            let gap = ((t - 0.45) * 1.3).max(0.0);
            if stripe >= gap {
                draw_rectangle(cx - half, y, half * 2.0, step, ramp(&SUN, t));
            }
            y += step;
        }
    }

    /// Floor, scrolling neon grid, road with lane markings, sun reflection.
    fn draw_ground(&self, ui: &Ui, cam: &Cam) {
        let s = ui.s;
        gradient_band(cam.crest_y, ui.h, ui.w, FLOOR_FAR, FLOOR_NEAR, 4.0);

        // Lines running toward the horizon, bending over the crest.
        for k in -20..=20 {
            let line = cam.ground_line(k as f32 * GRID_DX, NEAR_Z, CREST_Z);
            polyline(&line, 1.4 * s, GRID, 0.55);
        }
        // Cross lines, moving toward the viewer as we drive. They fade as
        // they bunch up near the crest.
        let phase = self.travel.rem_euclid(GRID_DZ);
        let mut z = NEAR_Z + GRID_DZ - phase;
        while z < CREST_Z {
            let y = cam.y(z);
            let near = (1.0 / z).min(1.0);
            let fade = 1.0 - smoothstep(CREST_Z * 0.6, CREST_Z, z);
            glow_line(vec2(0.0, y), vec2(ui.w, y), (0.8 + 1.4 * near) * s, GRID, (0.25 + 0.6 * near) * fade);
            z += GRID_DZ;
        }

        // Road surface, as strips so its edges can curve.
        let left = cam.ground_line(-ROAD_HALF, NEAR_Z, CREST_Z);
        let right = cam.ground_line(ROAD_HALF, NEAR_Z, CREST_Z);
        for i in 0..left.len() - 1 {
            draw_triangle(left[i], right[i], right[i + 1], ROAD);
            draw_triangle(left[i], right[i + 1], left[i + 1], ROAD);
        }

        // Wet-road reflection of the sun.
        let strips = 30;
        let depth = ui.h * 0.3;
        for i in 0..strips {
            let t = i as f32 / strips as f32;
            let y = cam.crest_y + t * depth;
            let half = ui.w * (0.02 + 0.06 * t);
            let shimmer = 0.6 + 0.4 * (y * 0.15 - self.time * 5.0).sin();
            let color = ramp(&SUN, 0.3 + 0.7 * t);
            let a = 0.16 * (1.0 - t) * shimmer;
            draw_rectangle(cam.cx - half, y, half * 2.0, depth / strips as f32 + 1.0, with_alpha(color, a));
        }

        polyline(&left, 2.0 * s, GRID, 0.9);
        polyline(&right, 2.0 * s, GRID, 0.9);
        for side in [-1.0, 1.0] {
            // Double yellow centre line.
            polyline(&cam.ground_line(side * 0.008, NEAR_Z, CREST_Z), 1.5 * s, CENTER_LINE, 0.8);
        }

        // Dashed lane lines, moving with the grid.
        let (gap, len) = (0.5, 0.22);
        let phase = self.travel.rem_euclid(gap);
        let mut z = NEAR_Z - phase;
        while z < CREST_Z {
            let (z0, z1) = (z.max(NEAR_Z * 0.5), (z + len).min(CREST_Z));
            for side in [-1.0, 1.0] {
                let x = side * ROAD_HALF * 0.5;
                let dash = [cam.ground(x, z0), cam.ground(x, (z0 + z1) / 2.0), cam.ground(x, z1)];
                polyline(&dash, 2.5 * s / z.max(0.5), LANE, 0.8);
            }
            z += gap;
        }
    }

    /// Wireframe mountain range beyond the crest, back row first so nearer
    /// rows hide the ones behind them.
    fn draw_mountains(&self, ui: &Ui, cam: &Cam) {
        let n = self.hist[0].len();
        // Column positions across the range (0..1), squeezed at the ends.
        let k = self.squeeze;
        let cols: Vec<f32> = (0..n)
            .map(|i| {
                let x = 2.0 * i as f32 / (n - 1) as f32 - 1.0;
                0.5 + 0.5 * (k * x).tanh() / k.tanh()
            })
            .collect();
        let rows: Vec<(f32, Vec<Vec2>)> = (0..MTN_ROWS)
            .map(|k| {
                let depth = k as f32 / (MTN_ROWS - 1) as f32;
                let z = CREST_Z + depth * (MTN_FAR_Z - CREST_Z);
                // Narrowing slower than true perspective keeps the range wide.
                let span = ui.w * MTN_SPAN * (CREST_Z / z).powf(0.35);
                // The front row sits flat on the crest; peaks rise behind it.
                let height = MTN_AMP * ui.h * (CREST_Z / z).sqrt() * smoothstep(0.0, 0.2, depth);
                let base = cam.y(z);
                // Blend toward the newer row so the ripple moves smoothly.
                let (cur, newer) = (&self.hist[k], &self.hist[k.saturating_sub(1)]);
                let blend = if k == 0 { 0.0 } else { self.clock };
                let points = (0..n)
                    .map(|i| {
                        let v = cur[i] + (newer[i] - cur[i]) * blend;
                        vec2(cam.cx + (cols[i] - 0.5) * span, base - v * height)
                    })
                    .collect();
                (depth, points)
            })
            .collect();

        let t = 1.3 * ui.s;
        for k in (0..MTN_ROWS - 1).rev() {
            let (depth, front) = (rows[k].0, &rows[k].1);
            let back = &rows[k + 1].1;
            let fill = mix(MTN_FILL_NEAR, MTN_FILL_FAR, depth);
            let line = mix(MTN_LINE, WHITE, self.kick * 0.6 * (1.0 - depth));
            // Fade lines out toward the crest so the range melts into the haze.
            let alpha = (1.0 - depth * 0.5) * smoothstep(0.0, 0.15, depth);
            for i in 0..n - 1 {
                let (a, b, c, d) = (front[i], front[i + 1], back[i + 1], back[i]);
                draw_triangle(a, b, c, fill);
                draw_triangle(a, c, d, fill);
            }
            for i in 0..n - 1 {
                glow_line(back[i], back[i + 1], t, line, alpha * 0.8);
                glow_line(front[i], front[i + 1], t, line, alpha);
            }
            for i in 0..n {
                glow_line(front[i], back[i], t, line, alpha * 0.8);
            }
        }
    }

    /// Two rows of palms beside the road, sliding toward the viewer.
    fn draw_palms(&mut self, ui: &Ui, cam: &Cam, f: &Frame) {
        let mut trees = Vec::new();
        for (side_idx, side, offset) in [(0, -1.0, 0.0), (1, 1.0, TREE_GAP / 2.0)] {
            let along = self.travel + offset;
            let first = (along / TREE_GAP).floor();
            let phase = along - first * TREE_GAP;
            // Palms that have passed the viewer are gone for good.
            self.palms.retain(|&(s, n), _| s != side_idx || n >= first as i64);
            for j in 0.. {
                let z = TREE_NEAR_Z + (j + 1) as f32 * TREE_GAP - phase;
                if z > CREST_Z {
                    break;
                }
                let key = (side_idx, first as i64 + j);
                let seed = (key.1 as u32).wrapping_mul(2).wrapping_add(side_idx as u32);
                let palm = *self.palms.entry(key).or_insert_with(|| Palm::grow(f, seed));
                trees.push((z, side, palm));
            }
        }
        // Farthest first.
        trees.sort_by(|a, b| b.0.total_cmp(&a.0));

        for (z, side, palm) in trees {
            let base = cam.ground(side * TREE_X, z);
            let size = TREE_H * ui.h / z;
            // Distant palms fade toward the haze colour.
            let color = mix(SILHOUETTE, TREE_FOG, 0.85 * smoothstep(TREE_FOG_FROM, CREST_Z, z));
            let sway = (self.time * 1.1 + palm.phase).sin() * 4.0 + self.kick * 6.0 * side;
            draw_palm(&palm, base, size, side, color, sway);
        }
    }
}

/// Soft glow band where the ground meets the mountains.
fn draw_haze(ui: &Ui, crest_y: f32) {
    let (above, below) = (ui.h * 0.12, ui.h * 0.06);
    let step = 2.0;
    let mut y = crest_y - above;
    while y < crest_y + below {
        let t = if y < crest_y { (y - (crest_y - above)) / above } else { 1.0 - (y - crest_y) / below };
        draw_rectangle(0.0, y, ui.w, step, with_alpha(HAZE, 0.45 * t * t));
        y += step;
    }
}

/// A palm silhouette: a tapered trunk leaning away from the road and a fan
/// of drooping, feathered fronds.
fn draw_palm(palm: &Palm, base: Vec2, size: f32, side: f32, color: Color, sway: f32) {
    let size = size * palm.height;
    let lean = side * palm.lean * size;
    let trunk = |t: f32| vec2(base.x + lean * t * t, base.y - size * t);
    let width = |t: f32| size * (0.04 - 0.018 * t);

    let segs = 10;
    for i in 0..segs {
        let (t0, t1) = (i as f32 / segs as f32, (i + 1) as f32 / segs as f32);
        let (a0, a1) = (trunk(t0), trunk(t1));
        let (w0, w1) = (width(t0), width(t1));
        // The trunk is close to vertical, so offsetting in x is enough.
        draw_triangle(a0 - vec2(w0, 0.0), a0 + vec2(w0, 0.0), a1 + vec2(w1, 0.0), color);
        draw_triangle(a0 - vec2(w0, 0.0), a1 + vec2(w1, 0.0), a1 - vec2(w1, 0.0), color);
    }

    let crown = trunk(1.0);
    let n = palm.fronds.max(2);
    for fi in 0..n {
        let jitter = (palm.phase + fi as f32 * 2.3).sin() * 6.0;
        let angle = -90.0 - palm.spread / 2.0 + fi as f32 * palm.spread / (n - 1) as f32 + sway + jitter;
        frond(crown, angle, size * palm.frond_len, size, palm.droop, palm.leaf, color);
    }
    draw_circle(crown.x, crown.y, size * 0.045, color);
}

/// One drooping frond: a curved stem with overlapping leaflet triangles on
/// both sides, widest in the middle.
fn frond(crown: Vec2, angle_deg: f32, len: f32, size: f32, droop: f32, leaf: f32, color: Color) {
    let a = angle_deg.to_radians();
    let dir = vec2(a.cos(), a.sin());
    let ctrl = crown + dir * len * 0.6 - vec2(0.0, len * 0.1);
    let end = crown + dir * len + vec2(0.0, len * droop * (0.4 + 0.6 * dir.x.abs()));
    let at = |t: f32| crown * (1.0 - t) * (1.0 - t) + ctrl * 2.0 * t * (1.0 - t) + end * t * t;

    let steps = 9;
    let mut prev = crown;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let p = at(t);
        let tangent = (p - prev).normalize_or_zero();
        let normal = vec2(-tangent.y, tangent.x);
        let length = size * leaf * (PI * t).sin().powf(0.6) * (1.0 - 0.3 * t);
        for sign in [-1.0, 1.0] {
            let d = (normal * sign * 0.9 + tangent * 0.5 + vec2(0.0, 0.5)).normalize_or_zero();
            draw_triangle(prev, p, p + d * length, color);
        }
        draw_line(prev.x, prev.y, p.x, p.y, size * 0.014 * (1.0 - 0.6 * t) + 0.5, color);
        prev = p;
    }
}

/// A line with a soft wider halo under it.
fn glow_line(a: Vec2, b: Vec2, thickness: f32, color: Color, alpha: f32) {
    draw_line(a.x, a.y, b.x, b.y, thickness * 3.0, with_alpha(color, 0.12 * alpha));
    draw_line(a.x, a.y, b.x, b.y, thickness, with_alpha(color, alpha));
}

fn polyline(points: &[Vec2], thickness: f32, color: Color, alpha: f32) {
    for w in points.windows(2) {
        glow_line(w[0], w[1], thickness, color, alpha);
    }
}

/// Vertical gradient from `top` to `bottom` colour between two y positions.
fn gradient_band(y0: f32, y1: f32, w: f32, top: Color, bottom: Color, step: f32) {
    let mut y = y0;
    while y < y1 {
        let t = (y - y0) / (y1 - y0);
        draw_rectangle(0.0, y, w, step + 0.5, mix(top, bottom, t));
        y += step;
    }
}

/// Lowers the very middle of the range a little, into a pass for the road.
fn pass(u: f32) -> f32 {
    1.0 - 0.35 * (1.0 - smoothstep(0.01, 0.06, (u - 0.5).abs()))
}

/// Fixed per-column height variation, so ridges look craggy, not smooth.
fn crag(i: usize) -> f32 {
    let mut rnd = xorshift(0x9e37_79b9 ^ (i as u32).wrapping_mul(0x85eb_ca6b));
    rnd();
    0.65 + 0.6 * rnd()
}

/// Tiny deterministic random number generator returning 0..1.
fn xorshift(mut seed: u32) -> impl FnMut() -> f32 {
    move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32
    }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
