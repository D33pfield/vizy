mod analysis;
mod audio;
mod render;

use macroquad::prelude::*;

use analysis::Analyzer;
use audio::Capture;
use render::{Renderer, Style};

const STYLE_KEYS: [KeyCode; 5] =
    [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4, KeyCode::Key5];

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

    let mut style = Style::Matrix;
    let mut analyzer = Analyzer::new(capture.sample_rate, style.bands(), style.freq_lo());
    let mut renderer = Renderer::new();
    renderer.style_changed();

    loop {
        if is_key_pressed(KeyCode::Q) || is_key_pressed(KeyCode::Escape) {
            break;
        }

        let picked = if is_key_pressed(KeyCode::Tab) {
            Some(style.next())
        } else {
            STYLE_KEYS.iter().position(|&k| is_key_pressed(k)).map(|i| Style::ALL[i])
        };
        if let Some(next) = picked {
            style = next;
            analyzer.configure(style.bands(), style.freq_lo());
            renderer.style_changed();
        }

        analyzer.update(&capture.shared, get_frame_time());
        renderer.draw(style, &analyzer.frame, &capture.device_name);
        next_frame().await;
    }
}
