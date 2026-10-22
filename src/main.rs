// Release builds open just the visualizer window, with no console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    macroquad::Window::from_config(vizy::window_conf(), vizy::run());
}
