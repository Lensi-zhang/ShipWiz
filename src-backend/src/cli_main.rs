fn main() {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("无法启动异步运行时：{error}");
            std::process::exit(1);
        }
    };
    let code = runtime.block_on(shipwiz_lib::cli::run());
    std::process::exit(code);
}
