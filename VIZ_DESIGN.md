# vizy: music visualizer (design brief)

Desktop music visualizer (native window, retro terminal/LED look) that shows whatever audio is playing on the PC,
with no virtual cables or third-party apps.

## Decisions so far

- **Language:** Rust (the user is new to Rust; Python is their usual language).
- **Runs on:** Windows, in its **own native window** drawn with `macroquad`.
  - Not a real terminal: running in a terminal (raw mode, resize handling,
    fonts, WSL interop) was judged too fragile. The look is emulated instead.
  - A terminal renderer could come back later as a second renderer over the same data.
- **Built from:** WSL2 Arch, cross-compiled to Windows.
- **Audio capture:** WASAPI loopback on the default output device, which is built into Windows.
  - One Windows `.exe` with no helper process. The WSL "pipe from a Windows
    helper" idea was dropped because it's too complex for now.
- **One scene: horizon** (`src/render/horizon.rs`), an 80s synthwave drive.
  The earlier styles (meter, matrix, mirror, chart) were removed; they live in
  git history at commit `290b657`.
  - Road with lane markings over a scrolling neon grid toward a striped sun
    (reflected on the wet road). The ground curves down like a small planet
    (crest = visible horizon); the road bends over it.
  - Wireframe mountain range beyond the crest, shaped by the spectrum (L half /
    R half, bass at the outer edges, small pass in the middle; older spectrum
    ripples back through the rows; sinks with distance; haze band at the seam).
    Peaks sharpened (power curve + per-column crag). Columns tanh-warped: even
    when quiet, ends squeezed / middle widened when loud.
  - Palms: small dense silhouettes fading into the haze, each grown from the
    music when it appears (bass = droop/length, treble = frond count/leaf
    fullness, mids = height/spread, plus per-tree randomness).
  - Drive speed follows loudness and surges on kicks; kicks pulse the sun and
    flash near ridges; treble twinkles the stars. Sky: stars drift slowly;
    random, slow shooting stars (every 4-16 s) with tapered white-to-violet trails.
    Sun stripes start just above its middle and thicken downward.
- **Tap effects (both views), meant to be played on the beat; each dies in
  ~0.2 s:** ↑ surge (mountains jump up to ~1.9x height, flash white, haze
  flares); ↓ ground wave (a swell rolls from the crest toward the viewer,
  lifting grid/road/palms; jolts the dash when it reaches the car; taps
  stack). Constants SURGE_*, WAVE_*, JOLT_DECAY in horizon.rs.
- **Two cameras, `c` toggles** with an eased 1.4 s swoop (`src/render/mod.rs`):
  - Chase view (default): above the road.
  - Cockpit (`src/render/dash.rs`): 80s supercar driver's seat. Camera low on
    the road with a wider lens, starting in the outer right lane;
    ←/→ signal a lane change (green dash arrow
    blinks); the change happens on the next deep bass hit (min 0.7 s, timeout
    5 s), opposite arrow cancels; glide between the four lanes (COCKPIT_*, LANES in horizon.rs), A-pillars, roof and
    rearview mirror close in, dashboard rises from below and dips on kicks.
    Displays: sweeping red scanner (speed follows energy), 3-column red voice
    box (L / mids / R), 7-segment MPH + bar (drive speed, ~1.5 s running average), bar tach (bass,
    redline flashes on kicks), amber VFD 16-band analyzer, button banks
    (soft backlit style; NORMAL CRUISE/PURSUIT by loudness with hysteresis,
    TURBO BOOST on strong kicks, others toggle slowly with treble), gullwing yoke. Road stays straight; switching is manual only.
- Analysis (`src/analysis.rs`) feeds the scene: per-channel RMS + spectrum,
  plus a mono average. The scene uses 40 bands per channel from 25 Hz to
  16 kHz (`render::BANDS`, `render::FREQ_LO`).
- **Dual FFT:** 4096 for most bands, 8192 for bands narrower than 2 small-FFT
  bins (the bass), for better low-end resolution. Bass lags ~40 ms more.
- **Tilt:** +4.5 dB/octave above 500 Hz only; bass is never cut. Auto-gain
  follows the loudest band, so this decides the bass/mid/treble balance.
  Knobs: `TILT_DB_PER_OCTAVE`, `TILT_PIVOT_HZ` in `analysis.rs`.

## Look and feel

Taken from the user's reference screenshots:

- **LED dot matrix:** every cell is a dot.
  - Lit dots are colored by height (bottom to top gradient, e.g. green → yellow → red, or cyan → yellow → red).
  - Unlit dots show as a faint dim grid.
- **Dot shape:** configurable (circle, square, rounded rect), drawn as shapes, not font glyphs.
- **Peak-hold dots:** fall slowly.
- **Mirror toggle (`m`):** bars grow up and down from a center line.
- **Frame:** rounded border with a title (btop-style). Later the title can show now-playing info.

## Future look: vacuum fluorescent display (VFD)

The user wants to eventually render like a 1980s hi-fi VFD panel:

- Spectrum bars made of stacked short horizontal segments, amber phosphor.
- Unlit segments faintly visible ("ghosting").
- Phosphor glow/bloom; mixed colors per region (amber, cyan-teal, red).
- 7/14-segment digit readouts (e.g. `16KHZ`, `87.80 MHz`), frequency labels
  under bars, small legends (`PEAK LEVEL METER`, `dB -∞ -20 ...`).
- Decorative outline graphics (tone/speaker diagrams).

**Architecture consequence:** analysis produces plain data (`Levels`: RMS,
bands, peaks); renderers only read it. LED matrix and VFD are two renderers.
The scene draws into an offscreen render target before hitting the screen,
so a bloom/glow shader pass can be inserted later. Segment digits will be
drawn as polygons (no font needed).

## Stack

| Purpose | Crate |
|---|---|
| Audio capture (WASAPI loopback) | `cpal` (input stream on the output device), or the `wasapi` crate if `cpal` gives trouble |
| FFT | `rustfft` |
| Window, drawing, key input, shaders | `macroquad` |

**Signal processing (DSP) chain:**

1. Mono downmix
2. Hann window
2. FFT
4. Log-spaced frequency bands
5. Auto-gain
6. Smoothing with fast attack and slow decay
7. Peak hold

**Threading:** the audio callback thread pushes samples into a ring buffer or channel, and the render loop runs at about 60 fps.

## Cross-compiling from WSL Arch

```bash
sudo pacman -S mingw-w64-gcc
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

- Copy the `.exe` to the Windows filesystem (e.g. `/mnt/c/Users/<you>/bin/`).
  It's faster to launch from there than from `\\wsl$`.
- `./run.sh` (or `./run.sh release`) builds, copies to `/mnt/c/Users/<you>/bin/`, and launches.
- `.cargo/config.toml` makes the Windows target the default, so plain `cargo build` works.
- `cargo-xwin` (MSVC target) is the fallback if the GNU target causes trouble.

## Gotchas

- WASAPI loopback delivers **no packets during silence**. Treat a timeout as
  silence; don't block the render loop.
- Apps using exclusive-mode audio can't be captured (rare).
- Handle changes to the default output device (e.g. speakers → headphones). Can come later.

## Build order

1. ~~Window + loopback capture + one RMS LED meter.~~ Done, works on Windows.
2. ~~FFT and log bands; style switching; stereo.~~ Done, works on Windows.
   Deeper bass range + dual FFT + horizon scene: done, works. Cockpit view: awaiting first run.
3. ~~Dot matrix, gradient colors, peak dots, rounded titled frame (the real look).
   Dot size, spacing and glow are free parameters (no cell grid).~~ Done.
4. Mirror toggle, color themes, keys (`q` quit, `m` mirror, `t` theme, `+/-` sensitivity, `[`/`]` bar count).

## Later ideas

- Spectrogram/waterfall mode, oscilloscope, stereo scope
- Beat detection: flash, particles
- Now-playing title and artist through Windows media controls (SMTC, via the `windows` crate)
- Palette taken from the album art
- Per-app loopback capture (Windows 10 2004+), e.g. `--app spotify`
- Native PipeWire source behind an `AudioSource` trait, for real Linux
