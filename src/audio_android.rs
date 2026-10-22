//! Android audio capture. For now a silent stand-in with the same interface
//! as the Windows loopback capture, so the scene runs while the real source
//! (Android's Visualizer API, which can hear Spotify) is built.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct Shared {
    pub left: VecDeque<f32>,
    pub right: VecDeque<f32>,
    /// When the last packet arrived; `None` means silent.
    pub last_packet: Option<Instant>,
}

pub struct Capture {
    pub shared: Arc<Mutex<Shared>>,
    pub sample_rate: u32,
    pub device_name: String,
}

impl Capture {
    pub fn start() -> Result<Self, String> {
        let shared = Shared { left: VecDeque::new(), right: VecDeque::new(), last_packet: None };
        Ok(Self {
            shared: Arc::new(Mutex::new(shared)),
            sample_rate: 48_000,
            device_name: "Android (no audio yet)".into(),
        })
    }
}
