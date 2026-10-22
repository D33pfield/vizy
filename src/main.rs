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
        Err(e) => {
            eprintln!("audio capture failed: {e}");
            std::process::exit(1);
        }
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
