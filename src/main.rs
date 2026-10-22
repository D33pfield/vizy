// Release builds open just the visualizer window, with no console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analysis;
mod audio;
mod render;

use macroquad::prelude::*;

use analysis::Analyzer;
use audio::Capture;
use render::Renderer;

fn window_conf() -> Conf {
    Conf {
        window_title: "vizy".into(),
        window_width: 960,
        window_height: 540,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let capture = match Capture::start() {
        Ok(c) => c,
        Err(e) => fail(&format!("Audio capture failed: {e}")),
    };
    println!("capturing: {} @ {} Hz", capture.device_name, capture.sample_rate);

    let mut analyzer = Analyzer::new(capture.sample_rate, render::BANDS, render::FREQ_LO);
    let mut renderer = Renderer::new();

    loop {
        if is_key_pressed(KeyCode::Q) || is_key_pressed(KeyCode::Escape) {
            break;
        }
        if is_key_pressed(KeyCode::C) {
            renderer.toggle_cockpit();
        }
        if is_key_pressed(KeyCode::Up) {
            renderer.surge();
        }
        if is_key_pressed(KeyCode::Down) {
            renderer.send_wave();
        }
        if is_key_pressed(KeyCode::Left) {
            renderer.change_lane(-1);
        }
        if is_key_pressed(KeyCode::Right) {
            renderer.change_lane(1);
        }

        analyzer.update(&capture.shared, get_frame_time());
        renderer.draw(&analyzer.frame);
        next_frame().await;
    }
}

/// Report a startup error and quit. Release builds have no console, so they
/// also show it in a message box.
fn fail(msg: &str) -> ! {
    eprintln!("{msg}");
    #[cfg(all(windows, not(debug_assertions)))]
    message_box(msg);
    std::process::exit(1);
}

#[cfg(all(windows, not(debug_assertions)))]
fn message_box(msg: &str) {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(hwnd: *mut std::ffi::c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }
    const MB_ICONERROR: u32 = 0x10;
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (text, caption) = (wide(msg), wide("vizy"));
    // SAFETY: both strings are NUL-terminated UTF-16 that outlive the call.
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_ICONERROR) };
}
