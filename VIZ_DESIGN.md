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
- **Styles** (switch with `Tab` or `1`-`5`; each is a file in `src/render/`):
  1. `meter`: stereo L/R LED level meter, shared dB scale.
  2. `matrix`: LED dot-matrix spectrum (mono), peak dots, frequency labels.
  3. `mirror`: square segments from a centre line, L up / R down, blue→red by distance, floating peaks, frameless.
  4. `chart`: btop-style two stacked panels (L, R), magenta grid, amber frame, blocky blue bars.
  5. `horizon` (Claude's own, refined with the user's synthwave refs): an 80s
     drive. Road with lane markings over a scrolling neon grid toward a
     striped sun (reflected on the wet road), palm trees sliding past on both
     sides, wireframe mountain range on the horizon shaped by the spectrum
     (L half / R half, bass at the outer edges, a pass in the middle for the
     road and sun; older spectrum ripples back through the rows). Drive speed
     follows loudness and surges on kicks; kicks pulse the sun and flash the
     near ridges; treble twinkles the stars. The ground curves down like a
     small planet (crest = visible horizon); the road bends over it and the
     mountain range sits beyond it, sinking with distance, blended by a haze
     band. Peaks sharpened (power curve + fixed per-column crag).
     Mountain columns are warped with tanh: even when quiet, ends squeezed
     and middle widened when loud (SQUEEZE_QUIET/LOUD/RATE). Palms are
     small dense silhouettes fading into the haze with distance, each grown
     from the music when it appears (bass = droop/length, treble = frond
     count/leaf fullness, mids = height/spread, plus per-tree randomness). Tuning
     constants at the top of `src/render/horizon.rs`.
- Analysis (`src/analysis.rs`) is shared by all styles: per-channel RMS + spectrum,
  plus a mono average. Each style asks for its own band count and low cutoff
  (`Style::bands`, `Style::freq_lo`): chart 20 Hz, mirror/horizon 25 Hz,
  matrix/meter 40 Hz. Top is 16 kHz.
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
   Deeper bass range + dual FFT + horizon style: awaiting first run.
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
