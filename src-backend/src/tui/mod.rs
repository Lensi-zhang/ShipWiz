mod app;
mod command;
mod input;
pub mod splash;
mod ui;

use crate::config::{self, NodeConnection, ShipNode};
use crate::docker::container;
use app::App;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, IsTerminal};
use std::time::{Duration, Instant};
use tokio::runtime::Handle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Containers,
    Images,
    Compose,
    Nodes,
    Registries,
    Help,
}

impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::Containers,
        Tab::Images,
        Tab::Compose,
        Tab::Nodes,
        Tab::Registries,
        Tab::Help,
    ];

    pub fn index(self) -> usize {
        Tab::ALL
            .iter()
            .position(|tab| *tab == self)
            .unwrap_or_default()
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Containers => "集装箱",
            Tab::Images => "图纸",
            Tab::Compose => "编队",
            Tab::Nodes => "节点",
            Tab::Registries => "补给港",
            Tab::Help => "帮助",
        }
    }

    fn next(self) -> Self {
        Tab::ALL[(self.index() + 1) % Tab::ALL.len()]
    }

    fn prev(self) -> Self {
        Tab::ALL[(self.index() + Tab::ALL.len() - 1) % Tab::ALL.len()]
    }
}

pub(super) fn split_lines(text: &str) -> Vec<String> {
    let lines = text
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        vec!["（无输出）".to_string()]
    } else {
        lines
    }
}

pub(super) fn address(node: &ShipNode) -> String {
    match node.connection {
        NodeConnection::Local => "本机 Docker Socket".to_string(),
        NodeConnection::Tls => format!("tcp://{}:{}", node.host, node.port),
        NodeConnection::Ssh => format!(
            "ssh://{}@{}:{}",
            node.username.as_deref().unwrap_or("-"),
            node.host,
            node.port
        ),
    }
}

pub(super) fn format_ports(ports: &[container::ContainerPort]) -> String {
    if ports.is_empty() {
        return "-".to_string();
    }
    ports
        .iter()
        .map(|port| match port.public_port {
            Some(public) => format!("{public}->{}/{}", port.private_port, port.protocol),
            None => format!("{}/{}", port.private_port, port.protocol),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn human_size(bytes: i64) -> String {
    let mut value = bytes.max(0) as f64;
    let mut unit = "B";
    for next in ["KB", "MB", "GB", "TB"] {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = next;
    }
    if unit == "B" {
        format!("{value:.0}{unit}")
    } else {
        format!("{value:.1}{unit}")
    }
}

pub(super) fn format_timestamp(ms: u64) -> String {
    let seconds = (ms / 1000) as i64;
    let days = seconds.div_euclid(86_400);
    let remaining = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        remaining / 3600,
        (remaining % 3600) / 60,
        remaining % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (year, month as u32, day as u32)
}

pub fn run(no_splash: bool) -> i32 {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("无法启动异步运行时：{error}");
            return 1;
        }
    };
    match run_loop(runtime.handle().clone(), no_splash) {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("{message}");
            1
        }
    }
}

fn should_play_splash(no_splash: bool) -> bool {
    let environment_disabled =
        std::env::var_os("SHIPWIZ_NO_SPLASH").is_some_and(|value| !value.is_empty());
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    should_play_splash_for(
        no_splash,
        environment_disabled,
        interactive,
        crossterm::terminal::size().ok(),
    )
}

fn should_play_splash_for(
    option_disabled: bool,
    environment_disabled: bool,
    interactive: bool,
    size: Option<(u16, u16)>,
) -> bool {
    !option_disabled
        && !environment_disabled
        && interactive
        && size.is_some_and(|(width, height)| width >= 80 && height >= 24)
}

fn run_loop(handle: Handle, no_splash: bool) -> Result<(), String> {
    let play_splash = should_play_splash(no_splash);
    let directory = config::config_directory().map_err(|error| error.message())?;
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("无法创建配置目录 {}：{error}", directory.display()))?;
    crate::ssh::init_known_hosts(directory.join("known_hosts.json"))
        .map_err(|error| error.message())?;

    enable_raw_mode().map_err(|error| format!("无法启用终端原始模式：{error}"))?;
    let mut stdout = io::stdout();
    if let Err(error) = execute!(stdout, EnterAlternateScreen, crossterm::cursor::Hide) {
        disable_raw_mode().ok();
        return Err(format!("无法进入全屏界面：{error}"));
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal =
        Terminal::new(backend).map_err(|error| format!("无法初始化终端：{error}"))?;
    terminal.clear().ok();

    // 启动动画：舰船破浪入场，播完自动交还画布
    if play_splash {
        splash::play(&mut terminal).ok();
    }

    let mut app = App::new(handle);
    let result = event_loop(&mut terminal, &mut app);

    disable_raw_mode().ok();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::cursor::Show
    )
    .ok();
    terminal.show_cursor().ok();
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), String> {
    const AUTO_REFRESH: Duration = Duration::from_secs(10);
    const POLL_INTERVAL: Duration = Duration::from_millis(100);

    let mut next_refresh = Instant::now() + AUTO_REFRESH;
    loop {
        app.poll_background();
        let now = Instant::now();
        if now >= next_refresh {
            if app.can_auto_refresh() {
                app.refresh_current_page();
                next_refresh = Instant::now() + AUTO_REFRESH;
            } else {
                // 弹层和耗时操作期间不刷新；恢复后再重新判断是否到期。
                next_refresh = now + POLL_INTERVAL;
            }
        }
        if app.take_dirty() {
            terminal
                .draw(|frame| ui::draw(frame, app))
                .map_err(|error| error.to_string())?;
        }
        if app.should_quit {
            return Ok(());
        }

        let timeout = next_refresh
            .saturating_duration_since(Instant::now())
            .min(POLL_INTERVAL);
        if event::poll(timeout).map_err(|error| error.to_string())? {
            match event::read().map_err(|error| error.to_string())? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.on_key(key);
                    app.show_pending_viewer();
                    app.mark_dirty();
                }
                Event::Resize(_, _) => app.mark_dirty(),
                _ => {}
            }
        }
        if app.should_quit {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::should_play_splash_for;

    #[test]
    fn splash_honors_explicit_and_environment_opt_outs() {
        let size = Some((100, 30));
        assert!(should_play_splash_for(false, false, true, size));
        assert!(!should_play_splash_for(true, false, true, size));
        assert!(!should_play_splash_for(false, true, true, size));
    }

    #[test]
    fn splash_requires_an_interactive_large_terminal() {
        assert!(!should_play_splash_for(
            false,
            false,
            false,
            Some((100, 30))
        ));
        assert!(!should_play_splash_for(false, false, true, Some((79, 24))));
        assert!(!should_play_splash_for(false, false, true, Some((80, 23))));
        assert!(should_play_splash_for(false, false, true, Some((80, 24))));
        assert!(!should_play_splash_for(false, false, true, None));
    }
}
