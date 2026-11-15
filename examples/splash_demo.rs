//! ShipWiz 启动动画独立预览（动画本体在 `tui::splash`）
//!
//! 运行演示：cargo run --example splash_demo
//! 定格某一帧便于截图：cargo run --example splash_demo -- --hold 1200
//! 只输出纯文本帧（便于审阅）：cargo run --example splash_demo -- --dump

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--dump") {
        shipwiz_lib::tui::splash::dump_frames();
        return Ok(());
    }
    let hold = args
        .iter()
        .position(|arg| arg == "--hold")
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse::<f64>().ok());
    shipwiz_lib::tui::splash::run_standalone(hold)
}
