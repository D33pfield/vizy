# vizy design notes

How vizy works and why it's built the way it is. For using it, see the
[README](../README.md).

## Goals

- Visualize whatever audio is playing on a Windows PC, with no virtual audio
  cables or helper apps.
- One small `.exe`, no installer, no runtime dependencies beyond Windows.
- A retro 80s look that reacts to the music in more ways than bar heights.

## Architecture

```
audio.rs ──samples──▶ analysis.rs ──Frame──▶ render/
(WASAPI loopback)     (FFT, bands, levels)    horizon.rs  the scene
                                              dash.rs     the cockpit dashboard
                                              mod.rs      cameras, swoop, text
```

- **Capture** (`src/audio.rs`): `cpal` opens an input stream on the default
  *output* device, which turns on WASAPI loopback. The audio thread only
  splits left/right and pushes samples into a shared buffer; nothing heavy
  runs there.
- **Analysis** (`src/analysis.rs`) runs once per frame on the render thread
  and produces a plain-data `Frame`: per-channel loudness, 40 log-spaced
  spectrum bands per channel (25 Hz to 16 kHz) and a mono average, all
  smoothed with peak hold.
- **Rendering** (`src/render/`) only reads the `Frame`. Everything is drawn
  from shapes in code (no image assets) into an offscreen target, which
  leaves room for a later bloom/glow shader pass.

## Signal processing

Per channel, every frame:

1. Take the newest samples (Hann window).
2. FFT at two sizes: 4096 for most bands, 8192 for bass bands narrower than
   two bins of the small FFT. Better bass resolution, at ~40 ms extra lag
   for the bass.
3. Group into log-spaced bands (strongest bin per band).
4. Tilt: +4.5 dB/octave above 500 Hz, so treble isn't drowned out. Bass is
   never cut.
5. Auto-gain follows the loudest band (instantly up, slowly down) and maps
   a 50 dB range to 0..1, with a floor so silence and noise stay small.
6. Fast-attack / slow-decay smoothing and a falling peak hold.

Loudness (RMS over 50 ms) feeds the level-based effects.

WASAPI loopback sends **no packets during silence**, so a 100 ms gap is
treated as silence and everything falls back to rest instead of freezing.

## The scene (`render/horizon.rs`)

An 80s synthwave drive toward a striped sun.

- **Ground:** perspective plus a downward bend with distance
  (`y = h0 + g/z + c·z`), so it curves away like a small planet. The crest
  is the visible horizon; the road, grid and palms bend over it.
- **Mountains:** a wireframe range beyond the crest, shaped by the spectrum.
  Left channel forms the left half and right the right half, bass at the
  outer edges, treble meeting in a small pass in the middle. Each row
  further back shows the spectrum a little later, so music ripples back
  through the range. Peaks are sharpened (power curve plus fixed per-column
  crag), and columns are tanh-warped: even when quiet, ends squeezed and
  middle widened when loud.
- **Palms:** silhouettes, each grown from the music the moment it appears
  (bass: droop and length; treble: frond count and leaf fullness; mids:
  height and spread; plus randomness), so the passing rows are a record of
  the song. They fade into the haze with distance.
- **Motion:** drive speed follows loudness and surges on kicks (a kick is the
  deep bass jumping above its recent average). Kicks also pulse the sun and
  flash the near ridges.
- **Sky:** gradient, slowly drifting stars that twinkle harder with treble,
  random slow shooting stars, haze band where the ground meets the
  mountains, and a smooth wet-road reflection of the sun.

## Cameras (`render/mod.rs`)

`c` swoops (eased, 1.4 s) between two cameras over the same scene:

- **Chase view:** above and behind, centred on the road.
- **Cockpit:** low on the road with a wider lens, in the outer right lane,
  sitting slightly left of the lane centre as the driver would.

## Cockpit dashboard (`render/dash.rs`)

An 80s supercar dash where every display is part of the visualizer:

- Scanner light sweeping across the glare shield, faster when busier.
- Three-column red voice box: left channel, mids, right channel.
- 7-segment MPH readout and bar: the drive speed, as a ~1.5 s average.
- Bar-graph tach driven by the bass; the redline flashes on kicks.
- Amber VFD 16-band spectrum analyzer with ghosted unlit segments.
- Backlit buttons: NORMAL CRUISE / PURSUIT by loudness (with hysteresis),
  TURBO BOOST on strong kicks, the rest toggling slowly with treble.
- Green turn signal arrows, A-pillars, roof, rearview mirror, gullwing yoke.
- The dash dips on kicks and jolts when a ground wave reaches the car.

## Interaction

Meant to be played on the beat:

- **↑ surge:** mountains jump to ~1.9x height, flash white, haze flares.
  Dies away in ~0.2 s.
- **↓ ground wave:** a swell rolls from the horizon to the viewer in ~0.5 s,
  lifting grid, road and palms. Taps stack.
- **←/→ (cockpit):** turn signal blinks; the lane change lands on the next
  deep bass hit (at least one blink, 5 s timeout). The opposite arrow
  cancels.

`Alt+Enter` or `F11` toggles borderless fullscreen; `Esc` leaves
fullscreen before it quits.

Tuning constants sit at the top of each file (`SURGE_*`, `WAVE_*`,
`LANE_*`, `COCKPIT_*`, `TILT_*`, and so on).

## Building from WSL

The project is developed in WSL2 and cross-compiled to Windows with the GNU
toolchain (`.cargo/config.toml` makes that the default target).
`./run.sh` builds, copies the `.exe` to your Windows `bin` folder (faster to
launch than from `\\wsl$`) and runs it; `./run.sh release` does the same
for an optimized build. `./release.sh` builds the release `.exe` and zips
it into `dist/`; it remaps the source paths Rust bakes into the binary so
the `.exe` doesn't contain your home folder or username.

## History

vizy started as a terminal visualizer, moved to a native window (terminals
were too fragile), and went through several styles: a stereo LED meter, a
dot-matrix spectrum, mirrored bars and a btop-style chart. Those were
removed when the synthwave scene took over; they're in git history at
commit `c1f0f45`.

## Ideas

- Now-playing title and artist through Windows media controls (SMTC).
- Follow default output device changes (speakers → headphones).
- Bloom/glow shader pass; palette from album art.
- More tap effects: sun flare, meteor shower, turbo boost, high beams.
- Per-app capture (Windows 10 2004+), e.g. only Spotify.
- Linux support via PipeWire behind an audio source trait.
