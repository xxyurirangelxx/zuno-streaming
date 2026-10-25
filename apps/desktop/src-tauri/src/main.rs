// Evita abrir um console no Windows em builds de release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    zuno_desktop_lib::run()
}
