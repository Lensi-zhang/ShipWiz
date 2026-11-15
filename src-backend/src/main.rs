#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = shipwiz_lib::run() {
        eprintln!("ShipWiz 启动失败：{error}");
        std::process::exit(1);
    }
}
