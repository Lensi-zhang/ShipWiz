fn print_help() {
    println!(
        "ShipWiz 终端舰队指挥台\n\n用法：shipwiz-cli [--no-splash]\n\n选项：\n    --no-splash    跳过启动动画（SHIPWIZ_NO_SPLASH 非空时效果相同）\n    -h, --help     显示此帮助\n\n非交互终端或终端小于 80×24 时也会自动跳过启动动画。"
    );
}

fn main() {
    let mut no_splash = false;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--no-splash" => no_splash = true,
            "-h" | "--help" => {
                print_help();
                return;
            }
            _ => {
                eprintln!("未知参数：{argument}\n使用 --help 查看用法。");
                std::process::exit(2);
            }
        }
    }
    std::process::exit(shipwiz_lib::tui::run(no_splash));
}
