use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Clear, Padding, Paragraph, Row, Table, Wrap,
};
use ratatui::Frame;

use super::app::{format_pull_layer, App, PullOperation};
use super::input::{Action, Overlay};
use super::{address, format_ports, format_timestamp, human_size, Tab};
use crate::config::NodeStatus;

const BG: Color = Color::Rgb(7, 21, 33);
const PANEL: Color = Color::Rgb(11, 29, 43);
const PANEL_ALT: Color = Color::Rgb(14, 36, 51);
const BORDER: Color = Color::Rgb(38, 70, 88);
const TEXT: Color = Color::Rgb(216, 230, 237);
const MUTED: Color = Color::Rgb(114, 143, 157);
const ACCENT: Color = Color::Rgb(67, 198, 217);
const GOLD: Color = Color::Rgb(227, 179, 65);
const GREEN: Color = Color::Rgb(88, 214, 141);
const RED: Color = Color::Rgb(255, 107, 107);
const SELECTION: Color = Color::Rgb(22, 78, 99);

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(BG)), area);

    if area.width < 60 || area.height < 16 {
        draw_compact(frame, area, app);
        if let Some(overlay) = app.overlay.as_mut() {
            draw_overlay(frame, area, overlay);
        }
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .split(area);

    draw_brand(frame, chunks[0]);
    draw_tabs(frame, chunks[1], app);
    draw_node_bar(frame, chunks[2], app);
    match app.tab {
        Tab::Containers => draw_containers(frame, chunks[3], app),
        Tab::Images => draw_images(frame, chunks[3], app),
        Tab::Compose => draw_compose(frame, chunks[3], app),
        Tab::Nodes => draw_nodes(frame, chunks[3], app),
        Tab::Registries => draw_registries(frame, chunks[3], app),
        Tab::Help => draw_help(frame, chunks[3]),
    }
    draw_status(frame, chunks[4], app);
    draw_command(frame, chunks[5], app);
    draw_hints(frame, chunks[6], app);

    if let Some(overlay) = app.overlay.as_mut() {
        draw_overlay(frame, area, overlay);
    }
}

fn draw_compact(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " ⚓ SHIPWIZ ",
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
            ),
            Span::styled("舰队指挥台", Style::default().fg(TEXT)),
        ])),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "◇ 舰桥空间不足",
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
            )),
            Line::from("请将终端扩展至至少 60 × 16，以显示完整航海仪表。"),
        ])
        .alignment(Alignment::Center)
        .style(Style::default().fg(MUTED).bg(BG)),
        chunks[1],
    );
    draw_command(frame, chunks[2], app);
    frame.render_widget(
        Paragraph::new(" q 退出").style(Style::default().fg(MUTED).bg(BG)),
        chunks[3],
    );
}

fn draw_brand(frame: &mut Frame, area: Rect) {
    let brand_area = Rect::new(area.x, area.y, area.width, 1);
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(36), Constraint::Length(28)])
        .split(brand_area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " ⚓ SHIPWIZ",
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  舰队指挥台",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  /  FLEET COMMAND", Style::default().fg(MUTED)),
        ]))
        .style(Style::default().bg(BG)),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("航海作业终端  ", Style::default().fg(MUTED)),
            Span::styled(
                format!("v{} ", env!("CARGO_PKG_VERSION")),
                Style::default().fg(ACCENT),
            ),
        ]))
        .alignment(Alignment::Right)
        .style(Style::default().bg(BG)),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new("─".repeat(area.width as usize)).style(Style::default().fg(BORDER).bg(BG)),
        Rect::new(area.x, area.y + 1, area.width, 1),
    );
}

fn tab_icon(tab: Tab) -> &'static str {
    match tab {
        Tab::Containers => "▣",
        Tab::Images => "◇",
        Tab::Compose => "≋",
        Tab::Nodes => "⚓",
        Tab::Registries => "▤",
        Tab::Help => "✦",
    }
}

fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let compact = area.width < 82;
    let current_only = area.width < 70;
    let mut spans = Vec::new();
    for (index, tab) in Tab::ALL.iter().enumerate() {
        if current_only && *tab != app.tab {
            continue;
        }
        let active = *tab == app.tab;
        let style = if active {
            Style::default()
                .fg(BG)
                .bg(GOLD)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(MUTED).bg(BG)
        };
        let label = if compact && !active {
            format!(" {} {} ", index + 1, tab_icon(*tab))
        } else {
            format!(" {} {} {} ", index + 1, tab_icon(*tab), tab.title())
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::styled(" ", Style::default().bg(BG)));
    }
    if current_only {
        spans.push(Span::styled(
            "  1–6 切换航海区域",
            Style::default().fg(MUTED).bg(BG),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(BG)),
        area,
    );
}

fn draw_node_bar(frame: &mut Frame, area: Rect, app: &App) {
    let mut left = Vec::new();
    match app.active_node() {
        Some(node) => {
            let (signal, status, color) = match node.status {
                NodeStatus::Online => ("●", "在线", GREEN),
                NodeStatus::Offline => ("○", "离线", RED),
            };
            let node_address = address(&node);
            left.extend([
                Span::styled(
                    " ◆ 旗舰 ",
                    Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    node.name,
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  {node_address}  "), Style::default().fg(MUTED)),
                Span::styled(
                    format!("{}  ", format!("{:?}", node.connection).to_uppercase()),
                    Style::default().fg(ACCENT),
                ),
                Span::styled(format!("{signal} {status}"), Style::default().fg(color)),
            ]);
        }
        None => left.extend([
            Span::styled(" ◇ 尚未指定旗舰 ", Style::default().fg(GOLD)),
            Span::styled("按 4 进入舰队页", Style::default().fg(MUTED)),
        ]),
    }

    if area.width >= 105 {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(64), Constraint::Percentage(36)])
            .split(area);
        frame.render_widget(
            Paragraph::new(Line::from(left)).style(Style::default().bg(BG)),
            chunks[0],
        );
        let running = app
            .containers
            .iter()
            .filter(|entry| entry.state.eq_ignore_ascii_case("running"))
            .count();
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("在航 ", Style::default().fg(MUTED)),
                Span::styled(
                    format!("{}/{}", running, app.containers.len()),
                    Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
                ),
                Span::styled("   图纸 ", Style::default().fg(MUTED)),
                Span::styled(app.images.len().to_string(), Style::default().fg(ACCENT)),
                Span::styled("   编队 ", Style::default().fg(MUTED)),
                Span::styled(
                    app.deployments.len().to_string(),
                    Style::default().fg(ACCENT),
                ),
                Span::raw(" "),
            ]))
            .alignment(Alignment::Right)
            .style(Style::default().bg(BG)),
            chunks[1],
        );
    } else {
        frame.render_widget(
            Paragraph::new(Line::from(left)).style(Style::default().bg(BG)),
            area,
        );
    }
}

fn panel(title: String, english: &'static str, symbol: &'static str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(PANEL))
        .title(Line::from(vec![
            Span::styled(
                format!(" {symbol} {title} "),
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("· {english} "), Style::default().fg(MUTED)),
        ]))
        .padding(Padding::new(1, 1, 0, 0))
}

fn overlay_panel(title: &str, english: &str, color: Color) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .style(Style::default().bg(PANEL).fg(TEXT))
        .title(Line::from(vec![
            Span::styled(
                format!(" ◆ {title} "),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("· {english} "), Style::default().fg(MUTED)),
        ]))
        .padding(Padding::new(1, 1, 0, 0))
}

fn row_style(selected: bool, index: usize) -> Style {
    if selected {
        Style::default()
            .fg(TEXT)
            .bg(SELECTION)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(TEXT)
            .bg(if index % 2 == 0 { PANEL } else { PANEL_ALT })
    }
}

fn header(cells: &[&str]) -> Row<'static> {
    Row::new(
        cells
            .iter()
            .map(|cell| Cell::from(cell.to_string()))
            .collect::<Vec<_>>(),
    )
    .style(
        Style::default()
            .fg(ACCENT)
            .bg(PANEL_ALT)
            .add_modifier(Modifier::BOLD),
    )
}

fn selection_marker(selected: bool) -> Cell<'static> {
    Cell::from(if selected { "▸" } else { "" }).style(Style::default().fg(GOLD))
}

fn right_cell(text: String) -> Cell<'static> {
    Cell::from(Line::from(text).alignment(Alignment::Right))
}

fn state_cell(state: &str) -> Cell<'static> {
    let (signal, color) = if state.eq_ignore_ascii_case("running") {
        ("●", GREEN)
    } else if state.eq_ignore_ascii_case("paused") || state.eq_ignore_ascii_case("restarting") {
        ("◆", GOLD)
    } else {
        ("○", MUTED)
    };
    Cell::from(Line::from(vec![
        Span::styled(format!("{signal} "), Style::default().fg(color)),
        Span::styled(state.to_string(), Style::default().fg(color)),
    ]))
}

fn node_state_cell(status: NodeStatus) -> Cell<'static> {
    let (signal, text, color) = match status {
        NodeStatus::Online => ("●", "在线", GREEN),
        NodeStatus::Offline => ("○", "离线", RED),
    };
    Cell::from(Line::from(vec![
        Span::styled(format!("{signal} "), Style::default().fg(color)),
        Span::styled(text, Style::default().fg(color)),
    ]))
}

fn draw_empty(frame: &mut Frame, area: Rect, block: Block<'static>, title: &str, hint: &str) {
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("≋ {title}"),
                Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(hint.to_string(), Style::default().fg(ACCENT))),
        ])
        .alignment(Alignment::Center)
        .style(Style::default().bg(PANEL)),
        inner,
    );
}

fn draw_containers(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = panel(
        format!("集装箱 · {} 艘", app.containers.len()),
        "CARGO DECK",
        "▣",
    );
    if app.containers.is_empty() {
        draw_empty(
            frame,
            area,
            block,
            "当前甲板暂无集装箱",
            "按 c 新建集装箱，开始首次航行",
        );
        return;
    }

    let show_details = area.width >= 105;
    let show_ports = area.width >= 120;
    let show_memory = area.width >= 88;
    let mut widths = vec![
        Constraint::Length(2),
        Constraint::Length(18),
        Constraint::Min(14),
        Constraint::Length(11),
    ];
    let mut headings = vec!["", "名称", "图纸", "状态"];
    if show_details {
        widths.push(Constraint::Length(18));
        headings.push("航行详情");
    }
    if show_ports {
        widths.push(Constraint::Length(18));
        headings.push("港口映射");
    }
    widths.push(Constraint::Length(8));
    headings.push("CPU");
    if show_memory {
        widths.push(Constraint::Length(10));
        headings.push("内存");
    }

    let selected = app.container_state.selected();
    let rows = app
        .containers
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let selected = Some(index) == selected;
            let mut cells = vec![
                selection_marker(selected),
                Cell::from(entry.name.clone()),
                Cell::from(entry.image.clone()),
                state_cell(&entry.state),
            ];
            if show_details {
                cells.push(Cell::from(entry.status.clone()).style(Style::default().fg(MUTED)));
            }
            if show_ports {
                cells.push(Cell::from(format_ports(&entry.ports)));
            }
            cells.push(right_cell(format!("{:.1}%", entry.cpu_percent)));
            if show_memory {
                cells.push(right_cell(if entry.memory_usage == 0 {
                    "-".to_string()
                } else {
                    human_size(entry.memory_usage as i64)
                }));
            }
            Row::new(cells).style(row_style(selected, index))
        })
        .collect::<Vec<_>>();
    let table = Table::new(rows, widths)
        .header(header(&headings))
        .block(block)
        .column_spacing(1);
    frame.render_stateful_widget(table, area, &mut app.container_state);
}

fn draw_images(frame: &mut Frame, area: Rect, app: &mut App) {
    let area = if let Some(operation) = app.pull_operation.as_ref() {
        let maximum = area.height.saturating_sub(8).max(5);
        let height = (operation.layers.len() as u16 + 4).clamp(5, maximum);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(height), Constraint::Min(3)])
            .split(area);
        draw_pull_progress(frame, chunks[0], operation);
        chunks[1]
    } else {
        area
    };
    let block = panel(
        format!("图纸 · {} 卷", app.images.len()),
        "CHART VAULT",
        "◇",
    );
    if app.images.is_empty() {
        draw_empty(
            frame,
            area,
            block,
            "图纸库暂无藏图",
            "按 p 采购图纸，或按 s 检索补给港",
        );
        return;
    }

    let show_created = area.width >= 105;
    let mut widths = vec![
        Constraint::Length(2),
        Constraint::Min(20),
        Constraint::Length(14),
        Constraint::Length(11),
    ];
    let mut headings = vec!["", "图纸", "识别码", "容量"];
    if show_created {
        widths.push(Constraint::Length(19));
        headings.push("绘制时间");
    }
    widths.push(Constraint::Length(10));
    headings.push("集装箱");

    let selected = app.image_state.selected();
    let rows = app
        .images
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let selected = Some(index) == selected;
            let tags = if entry.repo_tags.is_empty() {
                "<none>".to_string()
            } else {
                entry.repo_tags.join(", ")
            };
            let mut cells = vec![
                selection_marker(selected),
                Cell::from(tags),
                Cell::from(entry.short_id.clone()).style(Style::default().fg(MUTED)),
                right_cell(human_size(entry.size)),
            ];
            if show_created {
                cells.push(Cell::from(format_timestamp(
                    entry.created.max(0) as u64 * 1000,
                )));
            }
            cells.push(right_cell(entry.containers.to_string()));
            Row::new(cells).style(row_style(selected, index))
        })
        .collect::<Vec<_>>();
    let table = Table::new(rows, widths)
        .header(header(&headings))
        .block(block)
        .column_spacing(1);
    frame.render_stateful_widget(table, area, &mut app.image_state);
}

fn draw_pull_progress(frame: &mut Frame, area: Rect, operation: &PullOperation) {
    let block = panel(
        format!("采购 {}", operation.reference),
        "DOCKER PULL PROGRESS",
        "⇣",
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut lines = vec![Line::from(Span::styled(
        "正在后台接收 Docker 分层进度…",
        Style::default().fg(GOLD),
    ))];
    lines.extend(
        operation
            .layers
            .iter()
            .map(|layer| Line::from(format_pull_layer(layer))),
    );
    let scroll = lines.len().saturating_sub(inner.height as usize) as u16;
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(TEXT).bg(PANEL))
            .scroll((scroll, 0))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn draw_compose(frame: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let project_block = panel(
        format!("已存编队 · {}", app.projects.len()),
        "SQUADRON ARCHIVE",
        "≋",
    );
    if app.projects.is_empty() {
        draw_empty(
            frame,
            chunks[0],
            project_block,
            "暂无编队档案",
            "按 a 从 YAML 文件添加编队",
        );
    } else {
        let show_updated = chunks[0].width >= 90;
        let mut widths = vec![
            Constraint::Length(2),
            Constraint::Length(10),
            Constraint::Min(16),
        ];
        let mut headings = vec!["", "识别码", "编队名称"];
        if show_updated {
            widths.push(Constraint::Length(19));
            headings.push("更新时间");
        }
        let selected = app.project_state.selected();
        let rows = app
            .projects
            .iter()
            .enumerate()
            .map(|(index, project)| {
                let selected = Some(index) == selected;
                let mut cells = vec![
                    selection_marker(selected),
                    Cell::from(project.id.chars().take(8).collect::<String>())
                        .style(Style::default().fg(MUTED)),
                    Cell::from(project.name.clone()),
                ];
                if show_updated {
                    cells.push(Cell::from(format_timestamp(project.updated_time)));
                }
                Row::new(cells).style(row_style(selected, index))
            })
            .collect::<Vec<_>>();
        let table = Table::new(rows, widths)
            .header(header(&headings))
            .block(project_block)
            .column_spacing(1);
        frame.render_stateful_widget(table, chunks[0], &mut app.project_state);
    }

    let deployment_block = panel(
        format!("航行编队 · {}", app.deployments.len()),
        "ACTIVE SQUADRONS",
        "≋",
    );
    if app.deployments.is_empty() {
        draw_empty(
            frame,
            chunks[1],
            deployment_block,
            "当前没有航行中的编队",
            "选择已存编队后按 u 启动",
        );
    } else {
        let show_services = chunks[1].width >= 95;
        let mut widths = vec![Constraint::Min(16), Constraint::Length(12)];
        let mut headings = vec!["编队", "在航 / 总数"];
        if show_services {
            widths.push(Constraint::Min(18));
            headings.push("舰载服务");
        }
        widths.push(Constraint::Length(11));
        headings.push("状态");
        let rows = app
            .deployments
            .iter()
            .enumerate()
            .map(|(index, deployment)| {
                let mut cells = vec![
                    Cell::from(deployment.name.clone()),
                    right_cell(format!("{}/{}", deployment.running, deployment.containers)),
                ];
                if show_services {
                    cells.push(Cell::from(deployment.services.join(", ")));
                }
                cells.push(state_cell(&deployment.state));
                Row::new(cells).style(row_style(false, index))
            })
            .collect::<Vec<_>>();
        let table = Table::new(rows, widths)
            .header(header(&headings))
            .block(deployment_block)
            .column_spacing(1);
        frame.render_widget(table, chunks[1]);
    }
}

fn draw_nodes(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = panel(
        format!("舰队 · {} 艘", app.state.nodes.len()),
        "FLEET REGISTRY",
        "⚓",
    );
    if app.state.nodes.is_empty() {
        draw_empty(
            frame,
            area,
            block,
            "舰队中还没有舰艇",
            "按 a 新增本机或远程节点",
        );
        return;
    }

    let show_type = area.width >= 90;
    let show_connection = area.width >= 100;
    let show_last = area.width >= 125;
    let mut widths = vec![
        Constraint::Length(2),
        Constraint::Length(4),
        Constraint::Min(15),
    ];
    let mut headings = vec!["", "旗舰", "舰艇名称"];
    if show_type {
        widths.push(Constraint::Length(9));
        headings.push("舰种");
    }
    if show_connection {
        widths.push(Constraint::Length(8));
        headings.push("通道");
    }
    widths.push(Constraint::Min(18));
    headings.push("停泊地址");
    widths.push(Constraint::Length(9));
    headings.push("信号");
    if show_last {
        widths.push(Constraint::Length(19));
        headings.push("最近连通");
    }

    let current = app.state.current_node_id.clone();
    let selected = app.node_state.selected();
    let rows = app
        .state
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let selected = Some(index) == selected;
            let flagship = current.as_deref() == Some(node.id.as_str());
            let mut cells = vec![
                selection_marker(selected),
                Cell::from(if flagship { "◆" } else { "" }).style(Style::default().fg(GOLD)),
                Cell::from(node.name.clone()),
            ];
            if show_type {
                cells.push(Cell::from(format!("{:?}", node.node_type).to_lowercase()));
            }
            if show_connection {
                cells.push(
                    Cell::from(format!("{:?}", node.connection).to_uppercase())
                        .style(Style::default().fg(ACCENT)),
                );
            }
            cells.push(Cell::from(address(node)));
            cells.push(node_state_cell(node.status));
            if show_last {
                cells.push(Cell::from(
                    node.last_success_time
                        .map(format_timestamp)
                        .unwrap_or_else(|| "-".to_string()),
                ));
            }
            Row::new(cells).style(row_style(selected, index))
        })
        .collect::<Vec<_>>();
    let table = Table::new(rows, widths)
        .header(header(&headings))
        .block(block)
        .column_spacing(1);
    frame.render_stateful_widget(table, area, &mut app.node_state);
}

fn draw_registries(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = panel(
        format!("补给港 · {} 座", app.registries.len()),
        "SUPPLY PORTS",
        "▤",
    );
    if app.registries.is_empty() {
        draw_empty(
            frame,
            area,
            block,
            "尚未登记补给港",
            "按 a 新增图纸补给来源",
        );
        return;
    }

    let show_id = area.width >= 85;
    let mut widths = vec![Constraint::Length(2), Constraint::Length(5)];
    let mut headings = vec!["", "默认"];
    if show_id {
        widths.push(Constraint::Length(10));
        headings.push("识别码");
    }
    widths.push(Constraint::Length(18));
    headings.push("补给港");
    widths.push(Constraint::Min(24));
    headings.push("停泊地址");

    let selected = app.registry_state.selected();
    let rows = app
        .registries
        .iter()
        .enumerate()
        .map(|(index, registry)| {
            let selected = Some(index) == selected;
            let mut cells = vec![
                selection_marker(selected),
                Cell::from(if registry.is_default { "◆" } else { "" })
                    .style(Style::default().fg(GOLD)),
            ];
            if show_id {
                cells.push(
                    Cell::from(registry.id.chars().take(8).collect::<String>())
                        .style(Style::default().fg(MUTED)),
                );
            }
            cells.push(Cell::from(registry.name.clone()));
            cells.push(Cell::from(registry.url.clone()).style(Style::default().fg(ACCENT)));
            Row::new(cells).style(row_style(selected, index))
        })
        .collect::<Vec<_>>();
    let table = Table::new(rows, widths)
        .header(header(&headings))
        .block(block)
        .column_spacing(1);
    frame.render_stateful_widget(table, area, &mut app.registry_state);
}

fn help_title(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!("◆ {title}"),
        Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
    ))
}

fn help_entry(key: &str, description: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(" {key:<6}"),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(description.to_string(), Style::default().fg(TEXT)),
    ])
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let block = panel("舰桥手册".to_string(), "BRIDGE MANUAL", "✦");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if area.width < 100 {
        let lines = vec![
            help_title("全域航行"),
            help_entry("1–6", "切换区域 · Tab 循环 · F5 刷新 · q 退出"),
            help_entry("U", "强制检查更新（绕过缓存与跳过）"),
            help_entry("S / C", "更新弹层：跳过当前版本 / 恢复全部提醒"),
            Line::from(""),
            help_title("固定航令"),
            help_entry(":", "聚焦 · Enter 执行 · Esc 取消"),
            help_entry("指令", "1–6 · start/stop/restart/logs/rm"),
            help_entry("", "refresh · update · help · quit"),
            Line::from(""),
            help_title("甲板与图纸"),
            help_entry("s/x/r", "启航 / 停泊 / 重启集装箱"),
            help_entry("c/l/e", "新建 / 日志 / 执行命令"),
            help_entry("p/s", "采购 / 检索图纸"),
            Line::from(""),
            help_title("编队与舰队"),
            help_entry("u/D/T", "启动 / 停止编队 / 从模板创建"),
            help_entry("Enter/t", "切换旗舰 / 测试连接"),
            help_entry("a/e/d", "新增 / 编辑 / 移除"),
            Line::from(""),
            help_entry("Esc", "关闭弹层 · Enter 提交 · y/n 确认"),
        ];
        frame.render_widget(
            Paragraph::new(lines)
                .style(Style::default().bg(PANEL))
                .wrap(Wrap { trim: false }),
            inner,
        );
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);
    let operations = vec![
        help_title("航行操作 / OPERATIONS"),
        Line::from(""),
        help_entry("1–6 / Tab", "切换航海区域"),
        help_entry("F5 / ?", "刷新态势 / 打开手册"),
        help_entry("U", "检查更新 · 查看新版本说明"),
        Line::from(""),
        help_title("集装箱 / CARGO DECK"),
        help_entry("s / x / r", "启航 / 停泊 / 重启"),
        help_entry("d / l / e", "撤收 / 日志 / 执行命令"),
        help_entry("c / Enter", "新建 / 查看详情"),
        Line::from(""),
        help_title("图纸与编队"),
        help_entry("p / s / x", "采购 / 检索 / 导出图纸"),
        help_entry("i / d", "导入 / 移除图纸"),
        help_entry("u / D / l", "启动 / 停止编队 / 查看日志"),
        help_entry("T / a / d", "模板创建 / 添加 / 移除编队配置"),
    ];
    let protocols = vec![
        help_title("舰队规程 / FLEET PROTOCOL"),
        Line::from(""),
        help_title("舰队 / FLEET"),
        help_entry("Enter / t", "切换旗舰 / 测试连接"),
        help_entry("o / a / e", "概览 / 新增 / 编辑舰艇"),
        help_entry("d", "移除舰艇"),
        Line::from(""),
        help_title("补给港 / SUPPLY PORTS"),
        help_entry("a / u / d", "新增 / 设为默认 / 移除"),
        Line::from(""),
        help_title("弹层操作"),
        help_entry("Tab / ↑↓", "切换字段或选择记录"),
        help_entry("Enter / Esc", "提交 / 取消"),
        help_entry("y / n", "确认 / 取消危险操作"),
        Line::from(""),
        help_title("固定航令 / COMMANDS"),
        help_entry(":", "聚焦 · Enter 执行 · Esc 取消"),
        help_entry("1–6", "切换区域 · refresh / update / help / quit"),
        help_entry("容器", "start / stop / restart / logs / rm"),
        Line::from(""),
        Line::from(Span::styled(
            " ≋ 航令明确，舰队即刻响应。",
            Style::default().fg(MUTED),
        )),
    ];
    frame.render_widget(
        Paragraph::new(operations)
            .style(Style::default().bg(PANEL))
            .wrap(Wrap { trim: false }),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(protocols)
            .style(Style::default().bg(PANEL))
            .wrap(Wrap { trim: false }),
        chunks[1],
    );
}

fn draw_status(frame: &mut Frame, area: Rect, app: &App) {
    let text = if app.status.is_empty() {
        "全舰就绪"
    } else {
        app.status.as_str()
    };
    let color = if text.contains("失败") || text.contains("错误") || text.contains("无法") {
        RED
    } else if text.contains("正在") {
        GOLD
    } else {
        GREEN
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ≋ 舰桥日志 ", Style::default().fg(GOLD)),
            Span::styled("› ", Style::default().fg(MUTED)),
            Span::styled(text, Style::default().fg(color)),
        ]))
        .style(Style::default().bg(BG)),
        area,
    );
}

fn draw_command(frame: &mut Frame, area: Rect, app: &App) {
    let accent = if app.command_focused { GOLD } else { ACCENT };
    let mut spans = vec![
        Span::styled(
            " 航令 ",
            Style::default()
                .fg(accent)
                .bg(PANEL_ALT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("› ", Style::default().fg(accent).bg(PANEL_ALT)),
    ];
    if app.command_focused {
        spans.extend([
            Span::styled(
                app.command_input.clone(),
                Style::default().fg(TEXT).bg(PANEL_ALT),
            ),
            Span::styled("█", Style::default().fg(GOLD).bg(PANEL_ALT)),
            Span::styled(
                "  Enter 执行 · Esc 取消",
                Style::default().fg(MUTED).bg(PANEL_ALT),
            ),
        ]);
    } else {
        spans.extend([
            Span::styled(":", Style::default().fg(ACCENT).bg(PANEL_ALT)),
            Span::styled(
                " 输入航令 · help 查看指令",
                Style::default().fg(MUTED).bg(PANEL_ALT),
            ),
        ]);
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(PANEL_ALT)),
        area,
    );
}

fn push_hint(spans: &mut Vec<Span<'static>>, key: &str, description: &str) {
    spans.push(Span::styled(
        format!(" {key} "),
        Style::default()
            .fg(GOLD)
            .bg(PANEL_ALT)
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled(
        format!(" {description}  "),
        Style::default().fg(MUTED).bg(BG),
    ));
}

fn draw_hints(frame: &mut Frame, area: Rect, app: &App) {
    let hints: &[(&str, &str)] = match app.tab {
        Tab::Containers => &[
            ("s", "启航"),
            ("x", "停泊"),
            ("r", "重启"),
            ("d", "撤收"),
            ("l", "日志"),
            ("e", "执行"),
            ("c", "新建"),
            ("Enter", "详情"),
        ],
        Tab::Images => &[
            ("p", "采购"),
            ("s", "检索"),
            ("x", "导出"),
            ("i", "导入"),
            ("d", "移除"),
        ],
        Tab::Compose => &[
            ("u", "启动"),
            ("D", "停止"),
            ("l", "日志"),
            ("T", "模板"),
            ("a", "添加"),
            ("d", "移除"),
        ],
        Tab::Nodes => &[
            ("Enter", "切换旗舰"),
            ("t", "测连"),
            ("o", "概览"),
            ("a", "新增"),
            ("e", "编辑"),
            ("d", "移除"),
        ],
        Tab::Registries => &[("a", "新增"), ("u", "设默认"), ("d", "移除")],
        Tab::Help => &[("U", "检查更新"), ("?", "舰桥手册"), ("q", "退出")],
    };
    let mut spans = vec![Span::styled(" 航令 ", Style::default().fg(ACCENT))];
    let visible = if area.width < 95 { 4 } else { hints.len() };
    for (key, description) in hints.iter().take(visible.min(hints.len())) {
        push_hint(&mut spans, key, description);
    }
    if visible < hints.len() {
        spans.push(Span::styled("…  ", Style::default().fg(MUTED)));
    }
    let first = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(BG)),
        first,
    );

    let global = if area.width < 80 {
        Line::from(vec![
            Span::styled(" 1–6 ", Style::default().fg(ACCENT)),
            Span::styled("区域  ", Style::default().fg(MUTED)),
            Span::styled("F5 ", Style::default().fg(ACCENT)),
            Span::styled("刷新  ", Style::default().fg(MUTED)),
            Span::styled(": ", Style::default().fg(ACCENT)),
            Span::styled("航令  ", Style::default().fg(MUTED)),
            Span::styled("? ", Style::default().fg(ACCENT)),
            Span::styled("手册  ", Style::default().fg(MUTED)),
            Span::styled("q ", Style::default().fg(ACCENT)),
            Span::styled("退出", Style::default().fg(MUTED)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" 全域 ", Style::default().fg(ACCENT)),
            Span::styled(
                "1–6 区域   Tab 切换   F5 刷新   : 航令   U 更新   ? 舰桥手册   q 退出",
                Style::default().fg(MUTED),
            ),
        ])
    };
    frame.render_widget(
        Paragraph::new(global).style(Style::default().bg(BG)),
        Rect::new(area.x, area.y + 1, area.width, 1),
    );
}

fn centered(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1]);
    horizontal[1]
}

fn centered_rows(area: Rect, percent_x: u16, rows: u16) -> Rect {
    let height = rows.min(area.height);
    let width = area.width * percent_x / 100;
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

fn confirmation_color(action: &Action) -> Color {
    match action {
        Action::ContainerRemove { .. }
        | Action::ImageRemove { .. }
        | Action::ComposeRemove { .. }
        | Action::NodeRemove { .. }
        | Action::RegistryRemove { .. } => RED,
        _ => GOLD,
    }
}

fn draw_viewer(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    lines: &[String],
    scroll: u16,
    update: bool,
) {
    let popup = centered(area, 90, 85);
    frame.render_widget(Clear, popup);
    let text = lines
        .iter()
        .map(|line| Line::from(line.clone()))
        .collect::<Vec<_>>();
    let hint = if update {
        " ↑↓ 滚动 · S 跳过当前版本 · C 恢复全部提醒 · Esc 关闭 "
    } else {
        " ↑↓ 滚动 · Esc 关闭 "
    };
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(TEXT).bg(PANEL))
            .block(
                overlay_panel(title, "SHIP LOG", ACCENT)
                    .title_bottom(Line::from(hint).style(Style::default().fg(MUTED))),
            )
            .scroll((scroll, 0))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn draw_overlay(frame: &mut Frame, area: Rect, overlay: &mut Overlay) {
    match overlay {
        Overlay::Confirm {
            title,
            message,
            action,
        } => {
            let popup = centered_rows(area, 62, 7);
            let color = confirmation_color(action);
            frame.render_widget(Clear, popup);
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(message.clone(), Style::default().fg(TEXT))),
                Line::from(""),
                Line::from(vec![
                    Span::styled(" y / Enter ", Style::default().fg(color).bg(PANEL_ALT)),
                    Span::styled(" 确认    ", Style::default().fg(MUTED)),
                    Span::styled(" n / Esc ", Style::default().fg(ACCENT).bg(PANEL_ALT)),
                    Span::styled(" 取消", Style::default().fg(MUTED)),
                ]),
            ];
            frame.render_widget(
                Paragraph::new(lines)
                    .block(overlay_panel(title, "COMMAND CONFIRMATION", color))
                    .wrap(Wrap { trim: false }),
                popup,
            );
        }
        Overlay::Viewer {
            title,
            lines,
            scroll,
        } => draw_viewer(frame, area, title, lines, *scroll, false),
        Overlay::UpdateViewer {
            title,
            lines,
            scroll,
            ..
        } => draw_viewer(frame, area, title, lines, *scroll, true),
        Overlay::Form(form) => {
            let rows = (form.fields.len() as u16) * 2 + 3;
            let popup = centered_rows(area, 72, rows);
            frame.render_widget(Clear, popup);
            let mut lines = Vec::new();
            for (index, field) in form.fields.iter().enumerate() {
                let focused = index == form.focus;
                let style = if focused {
                    Style::default().fg(TEXT).bg(SELECTION)
                } else {
                    Style::default().fg(TEXT).bg(PANEL)
                };
                lines.push(
                    Line::from(vec![
                        Span::styled(
                            if focused { "▸ " } else { "  " },
                            Style::default().fg(if focused { GOLD } else { MUTED }),
                        ),
                        Span::styled(format!("{}：", field.label), Style::default().fg(ACCENT)),
                        Span::raw(field.value.clone()),
                    ])
                    .style(style),
                );
                lines.push(Line::from(""));
            }
            lines.push(Line::from(vec![
                Span::styled(" Tab/↑↓ ", Style::default().fg(GOLD).bg(PANEL_ALT)),
                Span::styled(" 切换  ", Style::default().fg(MUTED)),
                Span::styled(" Enter ", Style::default().fg(GOLD).bg(PANEL_ALT)),
                Span::styled(" 提交  ", Style::default().fg(MUTED)),
                Span::styled(" Esc ", Style::default().fg(ACCENT).bg(PANEL_ALT)),
                Span::styled(" 取消", Style::default().fg(MUTED)),
            ]));
            frame.render_widget(
                Paragraph::new(lines)
                    .block(overlay_panel(&form.title, "COMMAND INPUT", ACCENT))
                    .wrap(Wrap { trim: false }),
                popup,
            );
        }
        Overlay::Templates { templates, state } => {
            let popup = centered(area, 90, 78);
            frame.render_widget(Clear, popup);
            let block = overlay_panel("编队模板", "COMPOSE TEMPLATES", ACCENT).title_bottom(
                Line::from(" Enter 选择并输入项目名 · Esc 取消 ").style(Style::default().fg(MUTED)),
            );
            let inner = block.inner(popup);
            frame.render_widget(block, popup);
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(2), Constraint::Min(3)])
                .split(inner);
            let built_in = templates
                .iter()
                .filter(|template| template.built_in)
                .count();
            frame.render_widget(
                Paragraph::new(format!(
                    "{} 个模板 · {} 个内置 · {} 个自定义",
                    templates.len(),
                    built_in,
                    templates.len().saturating_sub(built_in)
                ))
                .style(Style::default().fg(TEXT)),
                chunks[0],
            );
            let selected = state.selected();
            let rows = templates.iter().enumerate().map(|(index, template)| {
                let selected = selected == Some(index);
                Row::new(vec![
                    selection_marker(selected),
                    Cell::from(if template.built_in {
                        "内置"
                    } else {
                        "自定义"
                    }),
                    Cell::from(format!("{} ({})", template.title, template.id)),
                    Cell::from(template.desc.clone()),
                    Cell::from(template.tag.join(" · ")),
                ])
                .style(row_style(selected, index))
            });
            let table = Table::new(
                rows,
                [
                    Constraint::Length(2),
                    Constraint::Length(7),
                    Constraint::Length(28),
                    Constraint::Min(26),
                    Constraint::Length(18),
                ],
            )
            .header(header(&["", "来源", "模板", "说明", "标签"]))
            .column_spacing(1);
            frame.render_stateful_widget(table, chunks[1], state);
        }
        Overlay::Search {
            term,
            results,
            state,
        } => {
            let popup = centered(area, 88, 82);
            frame.render_widget(Clear, popup);
            let block = overlay_panel("图纸检索", "CHART DISCOVERY", ACCENT);
            let inner = block.inner(popup);
            frame.render_widget(block, popup);
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2),
                    Constraint::Min(3),
                    Constraint::Length(1),
                ])
                .split(inner);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(" ◇ 检索海区  ", Style::default().fg(GOLD)),
                    Span::styled(term.clone(), Style::default().fg(TEXT)),
                    Span::styled(
                        format!("   发现 {} 卷图纸", results.len()),
                        Style::default().fg(MUTED),
                    ),
                ]))
                .style(Style::default().bg(PANEL)),
                chunks[0],
            );
            if results.is_empty() {
                frame.render_widget(
                    Paragraph::new("≋ 这片海区暂未发现匹配图纸")
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(MUTED).bg(PANEL)),
                    chunks[1],
                );
            } else {
                let widths = [
                    Constraint::Length(2),
                    Constraint::Min(20),
                    Constraint::Length(8),
                    Constraint::Length(8),
                    Constraint::Min(18),
                ];
                let selected = state.selected();
                let rows = results
                    .iter()
                    .enumerate()
                    .map(|(index, result)| {
                        let selected = Some(index) == selected;
                        Row::new(vec![
                            selection_marker(selected),
                            Cell::from(result.name.clone()),
                            right_cell(result.star_count.to_string()),
                            Cell::from(if result.is_official {
                                "◆ 官方"
                            } else {
                                "-"
                            })
                            .style(
                                Style::default().fg(if result.is_official { GOLD } else { MUTED }),
                            ),
                            Cell::from(result.description.clone()),
                        ])
                        .style(row_style(selected, index))
                    })
                    .collect::<Vec<_>>();
                let table = Table::new(rows, widths)
                    .header(header(&["", "图纸", "星标", "来源", "描述"]))
                    .style(Style::default().bg(PANEL))
                    .column_spacing(1);
                frame.render_stateful_widget(table, chunks[1], state);
            }
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(" ↑↓ ", Style::default().fg(GOLD).bg(PANEL_ALT)),
                    Span::styled(" 选择  ", Style::default().fg(MUTED)),
                    Span::styled(" Enter ", Style::default().fg(GOLD).bg(PANEL_ALT)),
                    Span::styled(" 采购  ", Style::default().fg(MUTED)),
                    Span::styled(" r ", Style::default().fg(GOLD).bg(PANEL_ALT)),
                    Span::styled(" 重检  ", Style::default().fg(MUTED)),
                    Span::styled(" Esc ", Style::default().fg(ACCENT).bg(PANEL_ALT)),
                    Span::styled(" 关闭", Style::default().fg(MUTED)),
                ]))
                .style(Style::default().bg(PANEL)),
                chunks[2],
            );
        }
    }
}
