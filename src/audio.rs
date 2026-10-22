//! Loopback capture: whatever is playing on the default output device.
//!
//! The audio callback runs on cpal's thread. It only splits out the left and
//! right channels and pushes them into a shared buffer; all analysis happens
//! on the render side.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{ErrorKind, FromSample, SampleFormat, SizedSample, Stream, StreamConfig};

/// Keep about this many seconds of samples around (per channel).
const BUFFER_SECONDS: f32 = 0.5;

pub struct Shared {
    pub left: VecDeque<f32>,
    /// Same as `left` for mono devices.
    pub right: VecDeque<f32>,
    /// When the last packet arrived. WASAPI loopback sends nothing during
    /// silence, so a stale timestamp means "silent", not "stuck".
    pub last_packet: Option<Instant>,
}

pub struct Capture {
    pub shared: Arc<Mutex<Shared>>,
    pub sample_rate: u32,
    pub device_name: String,
    // The stream stops when dropped, so Capture owns it for the app's lifetime.
    _stream: Stream,
}

impl Capture {
    pub fn start() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("no default output device")?;
        let device_name = device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "unknown device".into());

        // Opening an input stream on an *output* device turns on WASAPI loopback.
        let supported = device
            .default_output_config()
            .map_err(|e| format!("default output config: {e}"))?;
        let sample_rate = supported.sample_rate();
        let config = supported.config();

        let capacity = (sample_rate as f32 * BUFFER_SECONDS) as usize;
        let shared = Arc::new(Mutex::new(Shared {
            left: VecDeque::with_capacity(capacity),
            right: VecDeque::with_capacity(capacity),
            last_packet: None,
        }));

        let stream = match supported.sample_format() {
            SampleFormat::F32 => build::<f32>(&device, &config, shared.clone(), capacity),
            SampleFormat::I16 => build::<i16>(&device, &config, shared.clone(), capacity),
            SampleFormat::I32 => build::<i32>(&device, &config, shared.clone(), capacity),
            SampleFormat::U16 => build::<u16>(&device, &config, shared.clone(), capacity),
            other => return Err(format!("unsupported sample format {other}")),
        }?;
        stream.play().map_err(|e| format!("start stream: {e}"))?;

        Ok(Self {
            shared,
            sample_rate,
            device_name,
            _stream: stream,
        })
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    shared: Arc<Mutex<Shared>>,
    capacity: usize,
) -> Result<Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    device
        .build_input_stream::<T, _, _>(
            config.clone(),
            move |data: &[T], _| {
                let mut s = shared.lock().unwrap();
                // Frames are interleaved; on surround devices 0 and 1 are front L/R.
                for frame in data.chunks_exact(channels) {
                    let l = frame[0].to_sample::<f32>();
                    let r = if channels > 1 { frame[1].to_sample::<f32>() } else { l };
                    if s.left.len() == capacity {
                        s.left.pop_front();
                        s.right.pop_front();
                    }
                    s.left.push_back(l);
                    s.right.push_back(r);
                }
                s.last_packet = Some(Instant::now());
            },
            |e| {
                // Loopback flags a "discontinuity" whenever audio resumes after
                // silence. That's expected, not a real glitch.
                if e.kind() != ErrorKind::Xrun {
                    eprintln!("audio stream error: {e}");
                }
            },
            None,
        )
        .map_err(|e| format!("build input stream: {e}"))
}
