use super::app::App;
use super::command::{parse_command, ShipCommand};
use super::Tab;
use crate::config::{NodeConnection, NodeType};
use crate::docker::image;
use crate::template::ComposeTemplate;
use crate::update::UpdateInfo;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::TableState;

pub(super) struct FormField {
    pub(super) label: &'static str,
    pub(super) value: String,
}

pub(super) struct Form {
    pub(super) title: String,
    pub(super) kind: FormKind,
    pub(super) fields: Vec<FormField>,
    pub(super) focus: usize,
}

impl Form {
    fn new(title: impl Into<String>, kind: FormKind) -> Self {
        Self {
            title: title.into(),
            kind,
            fields: Vec::new(),
            focus: 0,
        }
    }

    fn field(mut self, label: &'static str, value: impl Into<String>) -> Self {
        self.fields.push(FormField {
            label,
            value: value.into(),
        });
        self
    }

    pub(super) fn value(&self, index: usize) -> String {
        self.fields
            .get(index)
            .map(|field| field.value.trim().to_string())
            .unwrap_or_default()
    }
}

#[derive(Clone)]
pub(super) enum FormKind {
    CreateContainer,
    Node { id: Option<String> },
    AddCompose,
    ComposeTemplate { template: ComposeTemplate },
    AddRegistry,
    PullImage,
    SearchImage,
    ExportImage { reference: String },
    ImportImage,
    Exec { id: String, name: String },
}

pub(super) enum Action {
    ContainerRemove { id: String, name: String },
    ImageRemove { id: String, name: String },
    ComposeUp { id: String },
    ComposeDown { id: String },
    ComposeRemove { id: String, name: String },
    NodeRemove { id: String, name: String },
    RegistryRemove { id: String, name: String },
    RegistryDefault { id: String, name: String },
}

pub(super) enum Overlay {
    Confirm {
        title: String,
        message: String,
        action: Action,
    },
    Form(Form),
    Viewer {
        title: String,
        lines: Vec<String>,
        scroll: u16,
    },
    UpdateViewer {
        title: String,
        lines: Vec<String>,
        scroll: u16,
        info: UpdateInfo,
    },
    Templates {
        templates: Vec<ComposeTemplate>,
        state: TableState,
    },
    Search {
        term: String,
        results: Vec<image::ImageSearchResult>,
        state: TableState,
    },
}

impl App {
    pub(super) fn on_key(&mut self, key: KeyEvent) {
        if let Some(overlay) = self.overlay.take() {
            self.handle_overlay(overlay, key);
            return;
        }

        if self.command_focused {
            match key.code {
                KeyCode::Esc => {
                    self.command_input.clear();
                    self.command_focused = false;
                }
                KeyCode::Enter => {
                    let input = std::mem::take(&mut self.command_input);
                    self.command_focused = false;
                    self.execute_command(&input);
                }
                KeyCode::Backspace => {
                    self.command_input.pop();
                }
                KeyCode::Char(character) => self.command_input.push(character),
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char(':') => {
                self.command_focused = true;
                return;
            }
            KeyCode::Char('q') => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('?') => {
                self.tab = Tab::Help;
                return;
            }
            KeyCode::Char('U') => {
                self.start_update_check(true);
                return;
            }
            KeyCode::F(5) => {
                // 不设置通用“已刷新”，保留原状态以及本次刷新产生的错误。
                self.refresh_all();
                return;
            }
            KeyCode::Tab => {
                self.tab = self.tab.next();
                return;
            }
            KeyCode::BackTab => {
                self.tab = self.tab.prev();
                return;
            }
            KeyCode::Char('1') => {
                self.tab = Tab::Containers;
                return;
            }
            KeyCode::Char('2') => {
                self.tab = Tab::Images;
                return;
            }
            KeyCode::Char('3') => {
                self.tab = Tab::Compose;
                return;
            }
            KeyCode::Char('4') => {
                self.tab = Tab::Nodes;
                return;
            }
            KeyCode::Char('5') => {
                self.tab = Tab::Registries;
                return;
            }
            KeyCode::Char('6') => {
                self.tab = Tab::Help;
                return;
            }
            _ => {}
        }

        match self.tab {
            Tab::Containers => self.container_keys(key.code),
            Tab::Images => self.image_keys(key.code),
            Tab::Compose => self.compose_keys(key.code),
            Tab::Nodes => self.node_keys(key.code),
            Tab::Registries => self.registry_keys(key.code),
            Tab::Help => {}
        }
    }

    fn execute_command(&mut self, input: &str) {
        let command = match parse_command(input) {
            Ok(Some(command)) => command,
            Ok(None) => return,
            Err(error) => {
                self.status = error;
                return;
            }
        };

        match command {
            ShipCommand::SwitchTab(tab) => self.tab = tab,
            ShipCommand::Start => {
                if let Some((id, name)) = self.selected_container() {
                    self.start_container(id, name);
                } else {
                    self.status = "请先在集装箱页选择目标".to_string();
                }
            }
            ShipCommand::Stop => {
                if let Some((id, name)) = self.selected_container() {
                    self.stop_container(id, name);
                } else {
                    self.status = "请先在集装箱页选择目标".to_string();
                }
            }
            ShipCommand::Restart => {
                if let Some((id, name)) = self.selected_container() {
                    self.restart_container(id, name);
                } else {
                    self.status = "请先在集装箱页选择目标".to_string();
                }
            }
            ShipCommand::Logs => {
                if let Some((id, name)) = self.selected_container() {
                    self.show_logs(id, name);
                } else {
                    self.status = "请先在集装箱页选择目标".to_string();
                }
            }
            ShipCommand::Remove => {
                if let Some((id, name)) = self.selected_container() {
                    self.overlay = Some(Overlay::Confirm {
                        title: "撤收集装箱".to_string(),
                        message: format!("确认强制移除集装箱 {name}？"),
                        action: Action::ContainerRemove { id, name },
                    });
                } else {
                    self.status = "请先在集装箱页选择目标".to_string();
                }
            }
            ShipCommand::Refresh => self.refresh_all(),
            ShipCommand::Help => self.tab = Tab::Help,
            ShipCommand::Update => self.start_update_check(true),
            ShipCommand::Quit => self.should_quit = true,
        }
    }

    fn container_keys(&mut self, code: KeyCode) {
        let len = self.containers.len();
        match code {
            KeyCode::Enter => self.show_container_detail(),
            KeyCode::Char('s') => {
                if let Some((id, name)) = self.selected_container() {
                    self.start_container(id, name);
                }
            }
            KeyCode::Char('x') => {
                if let Some((id, name)) = self.selected_container() {
                    self.stop_container(id, name);
                }
            }
            KeyCode::Char('r') => {
                if let Some((id, name)) = self.selected_container() {
                    self.restart_container(id, name);
                }
            }
            KeyCode::Char('d') => {
                if let Some((id, name)) = self.selected_container() {
                    self.overlay = Some(Overlay::Confirm {
                        title: "撤收集装箱".to_string(),
                        message: format!("确认强制移除集装箱 {name}？"),
                        action: Action::ContainerRemove { id, name },
                    });
                }
            }
            KeyCode::Char('l') => {
                if let Some((id, name)) = self.selected_container() {
                    self.show_logs(id, name);
                }
            }
            KeyCode::Char('e') => {
                if let Some((id, name)) = self.selected_container() {
                    let form = Form::new(
                        format!("在 {name} 内执行命令"),
                        FormKind::Exec {
                            id,
                            name: name.clone(),
                        },
                    )
                    .field("命令", "");
                    self.overlay = Some(Overlay::Form(form));
                }
            }
            KeyCode::Char('c') => {
                let form = Form::new("新建集装箱", FormKind::CreateContainer)
                    .field("名称", "")
                    .field("图纸", "")
                    .field("命令（可空）", "")
                    .field("环境变量 K=V,K=V（可空）", "")
                    .field("端口 host:port,host:port（可空）", "");
                self.overlay = Some(Overlay::Form(form));
            }
            _ => move_selection(&mut self.container_state, len, code),
        }
    }

    fn image_keys(&mut self, code: KeyCode) {
        let len = self.images.len();
        match code {
            KeyCode::Char('p') => {
                let form = Form::new("采购图纸", FormKind::PullImage).field("图纸名称与标签", "");
                self.overlay = Some(Overlay::Form(form));
            }
            KeyCode::Char('s') => {
                let form = Form::new("检索图纸", FormKind::SearchImage).field("检索关键词", "");
                self.overlay = Some(Overlay::Form(form));
            }
            KeyCode::Char('x') => {
                if let Some(index) = self.image_state.selected().filter(|index| *index < len) {
                    let entry = &self.images[index];
                    let reference = entry
                        .repo_tags
                        .first()
                        .cloned()
                        .unwrap_or_else(|| entry.short_id.clone());
                    let form = Form::new(
                        format!("导出 {reference}"),
                        FormKind::ExportImage {
                            reference: reference.clone(),
                        },
                    )
                    .field("导出文件路径", "");
                    self.overlay = Some(Overlay::Form(form));
                }
            }
            KeyCode::Char('i') => {
                let form = Form::new("导入图纸", FormKind::ImportImage).field("tar 文件路径", "");
                self.overlay = Some(Overlay::Form(form));
            }
            KeyCode::Char('d') => {
                if let Some(index) = self.image_state.selected().filter(|index| *index < len) {
                    let entry = &self.images[index];
                    let id = entry.id.clone();
                    let name = entry
                        .repo_tags
                        .first()
                        .cloned()
                        .unwrap_or_else(|| entry.short_id.clone());
                    self.overlay = Some(Overlay::Confirm {
                        title: "移除图纸".to_string(),
                        message: format!("确认移除图纸 {name}？"),
                        action: Action::ImageRemove { id, name },
                    });
                }
            }
            _ => move_selection(&mut self.image_state, len, code),
        }
    }

    fn compose_keys(&mut self, code: KeyCode) {
        let len = self.projects.len();
        match code {
            KeyCode::Char('T') => self.open_compose_templates(),
            KeyCode::Char('u') => {
                if let Some(index) = self.project_state.selected().filter(|index| *index < len) {
                    let id = self.projects[index].id.clone();
                    self.overlay = Some(Overlay::Confirm {
                        title: "启动编队".to_string(),
                        message: format!("确认启动编队 {}？", self.projects[index].name),
                        action: Action::ComposeUp { id },
                    });
                }
            }
            KeyCode::Char('D') => {
                if let Some(index) = self.project_state.selected().filter(|index| *index < len) {
                    let id = self.projects[index].id.clone();
                    self.overlay = Some(Overlay::Confirm {
                        title: "停止编队".to_string(),
                        message: format!("确认停止编队 {}？", self.projects[index].name),
                        action: Action::ComposeDown { id },
                    });
                }
            }
            KeyCode::Char('l') => {
                if let Some(index) = self.project_state.selected().filter(|index| *index < len) {
                    let name = self.projects[index].name.clone();
                    self.show_compose_logs(name);
                }
            }
            KeyCode::Char('a') => {
                let form = Form::new("添加编队", FormKind::AddCompose)
                    .field("编队名称", "")
                    .field("YAML 文件路径", "");
                self.overlay = Some(Overlay::Form(form));
            }
            KeyCode::Char('d') => {
                if let Some(index) = self.project_state.selected().filter(|index| *index < len) {
                    let id = self.projects[index].id.clone();
                    let name = self.projects[index].name.clone();
                    self.overlay = Some(Overlay::Confirm {
                        title: "移除编队".to_string(),
                        message: format!("确认从舰队配置中移除编队 {name}？"),
                        action: Action::ComposeRemove { id, name },
                    });
                }
            }
            _ => move_selection(&mut self.project_state, len, code),
        }
    }

    fn node_keys(&mut self, code: KeyCode) {
        let len = self.state.nodes.len();
        match code {
            KeyCode::Enter => self.use_node(),
            KeyCode::Char('t') => self.test_node(),
            KeyCode::Char('o') => self.show_node_overview(),
            KeyCode::Char('a') => {
                let form = Form::new("新增舰船", FormKind::Node { id: None })
                    .field("名称", "")
                    .field("备注", "")
                    .field("类型 local|remote", "local")
                    .field("通道 local|tls|ssh", "local")
                    .field("主机", "localhost")
                    .field("端口", "0")
                    .field("TLS 证书目录", "")
                    .field("SSH 用户名", "")
                    .field("SSH 密码", "");
                self.overlay = Some(Overlay::Form(form));
            }
            KeyCode::Char('e') => {
                if let Some(index) = self.node_state.selected().filter(|index| *index < len) {
                    let node = self.state.nodes[index].clone();
                    let form = Form::new(
                        format!("编辑舰船 {}", node.name),
                        FormKind::Node {
                            id: Some(node.id.clone()),
                        },
                    )
                    .field("名称", node.name.clone())
                    .field("备注", node.alias.clone())
                    .field(
                        "类型 local|remote",
                        if node.node_type == NodeType::Local {
                            "local"
                        } else {
                            "remote"
                        },
                    )
                    .field(
                        "通道 local|tls|ssh",
                        match node.connection {
                            NodeConnection::Local => "local",
                            NodeConnection::Tls => "tls",
                            NodeConnection::Ssh => "ssh",
                        },
                    )
                    .field("主机", node.host.clone())
                    .field("端口", node.port.to_string())
                    .field("TLS 证书目录", node.cert_path.clone().unwrap_or_default())
                    .field("SSH 用户名", node.username.clone().unwrap_or_default())
                    .field("SSH 密码", "");
                    self.overlay = Some(Overlay::Form(form));
                }
            }
            KeyCode::Char('d') => {
                if let Some(index) = self.node_state.selected().filter(|index| *index < len) {
                    let node = &self.state.nodes[index];
                    self.overlay = Some(Overlay::Confirm {
                        title: "移除舰船".to_string(),
                        message: format!("确认将 {}（{}）移出舰队？", node.name, node.host),
                        action: Action::NodeRemove {
                            id: node.id.clone(),
                            name: node.name.clone(),
                        },
                    });
                }
            }
            _ => move_selection(&mut self.node_state, len, code),
        }
    }

    fn registry_keys(&mut self, code: KeyCode) {
        let len = self.registries.len();
        match code {
            KeyCode::Char('a') => {
                let form = Form::new("新增补给港", FormKind::AddRegistry)
                    .field("名称", "")
                    .field("地址 https://…", "");
                self.overlay = Some(Overlay::Form(form));
            }
            KeyCode::Char('u') | KeyCode::Enter => {
                if let Some(index) = self.registry_state.selected().filter(|index| *index < len) {
                    let registry = &self.registries[index];
                    self.overlay = Some(Overlay::Confirm {
                        title: "切换默认补给港".to_string(),
                        message: format!("确认把 {} 设为默认补给港？", registry.name),
                        action: Action::RegistryDefault {
                            id: registry.id.clone(),
                            name: registry.name.clone(),
                        },
                    });
                }
            }
            KeyCode::Char('d') => {
                if let Some(index) = self.registry_state.selected().filter(|index| *index < len) {
                    let registry = &self.registries[index];
                    if registry.id == "docker-hub" {
                        self.status = "官方补给港不可移除".to_string();
                    } else {
                        self.overlay = Some(Overlay::Confirm {
                            title: "移除补给港".to_string(),
                            message: format!("确认移除补给港 {}？", registry.name),
                            action: Action::RegistryRemove {
                                id: registry.id.clone(),
                                name: registry.name.clone(),
                            },
                        });
                    }
                }
            }
            _ => move_selection(&mut self.registry_state, len, code),
        }
    }

    fn handle_overlay(&mut self, overlay: Overlay, key: KeyEvent) {
        match overlay {
            Overlay::Viewer {
                title,
                lines,
                scroll,
            } => {
                self.overlay =
                    next_viewer_scroll(key.code, scroll, lines.len()).map(|next| Overlay::Viewer {
                        title,
                        lines,
                        scroll: next,
                    });
            }
            Overlay::UpdateViewer {
                title,
                lines,
                scroll,
                info,
            } => match key.code {
                KeyCode::Char('S') => self.skip_current_update(info),
                KeyCode::Char('C') => self.restore_update_notifications(info),
                _ => {
                    self.overlay = next_viewer_scroll(key.code, scroll, lines.len()).map(|next| {
                        Overlay::UpdateViewer {
                            title,
                            lines,
                            scroll: next,
                            info,
                        }
                    });
                }
            },
            Overlay::Templates {
                templates,
                mut state,
            } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.overlay = None,
                KeyCode::Up
                | KeyCode::Char('k')
                | KeyCode::Down
                | KeyCode::Char('j')
                | KeyCode::PageUp
                | KeyCode::PageDown
                | KeyCode::Home
                | KeyCode::End => {
                    move_selection(&mut state, templates.len(), key.code);
                    self.overlay = Some(Overlay::Templates { templates, state });
                }
                KeyCode::Enter => {
                    let picked = state
                        .selected()
                        .filter(|index| *index < templates.len())
                        .map(|index| templates[index].clone());
                    self.overlay = picked.map(|template| {
                        let form = Form::new(
                            format!("使用模板 · {}", template.title),
                            FormKind::ComposeTemplate {
                                template: template.clone(),
                            },
                        )
                        .field("项目名", "");
                        Overlay::Form(form)
                    });
                }
                _ => self.overlay = Some(Overlay::Templates { templates, state }),
            },
            Overlay::Confirm {
                title,
                message,
                action,
            } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.overlay = None;
                    self.execute(action);
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => self.overlay = None,
                _ => {
                    self.overlay = Some(Overlay::Confirm {
                        title,
                        message,
                        action,
                    })
                }
            },
            Overlay::Form(mut form) => {
                if form.fields.is_empty() {
                    self.overlay = None;
                    return;
                }
                match key.code {
                    KeyCode::Esc => {
                        self.overlay = None;
                        return;
                    }
                    KeyCode::Tab | KeyCode::Down => {
                        form.focus = (form.focus + 1) % form.fields.len();
                    }
                    KeyCode::BackTab | KeyCode::Up => {
                        form.focus = (form.focus + form.fields.len() - 1) % form.fields.len();
                    }
                    KeyCode::Backspace => {
                        form.fields[form.focus].value.pop();
                    }
                    KeyCode::Char(character) => {
                        form.fields[form.focus].value.push(character);
                    }
                    KeyCode::Enter => {
                        self.overlay = None;
                        self.submit_form(form);
                        return;
                    }
                    _ => {}
                }
                self.overlay = Some(Overlay::Form(form));
            }
            Overlay::Search {
                term,
                results,
                mut state,
            } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.overlay = None,
                KeyCode::Up | KeyCode::Char('k') | KeyCode::Down | KeyCode::Char('j') => {
                    move_selection(&mut state, results.len(), key.code);
                    self.overlay = Some(Overlay::Search {
                        term,
                        results,
                        state,
                    });
                }
                KeyCode::Enter => {
                    let picked = state
                        .selected()
                        .filter(|index| *index < results.len())
                        .map(|index| results[index].name.clone());
                    self.overlay = None;
                    if let Some(reference) = picked {
                        self.pull_image(reference);
                    }
                }
                KeyCode::Char('r') => {
                    self.overlay = None;
                    self.run_search(term);
                }
                _ => {
                    self.overlay = Some(Overlay::Search {
                        term,
                        results,
                        state,
                    });
                }
            },
        }
    }

    fn execute(&mut self, action: Action) {
        match action {
            Action::ContainerRemove { id, name } => self.remove_container(id, name),
            Action::ImageRemove { id, name } => self.remove_image(id, name),
            Action::ComposeUp { id } => self.compose_action(id, true),
            Action::ComposeDown { id } => self.compose_action(id, false),
            Action::ComposeRemove { id, name } => self.remove_project(id, name),
            Action::NodeRemove { id, name } => self.remove_node(id, name),
            Action::RegistryRemove { id, name } => self.remove_registry(id, name),
            Action::RegistryDefault { id, name } => self.set_default_registry(id, name),
        }
    }

    fn selected_container(&self) -> Option<(String, String)> {
        let index = self
            .container_state
            .selected()
            .filter(|index| *index < self.containers.len())?;
        let entry = &self.containers[index];
        Some((entry.id.clone(), entry.name.clone()))
    }
}

fn next_viewer_scroll(code: KeyCode, scroll: u16, line_count: usize) -> Option<u16> {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => None,
        KeyCode::Up | KeyCode::Char('k') => Some(scroll.saturating_sub(1)),
        KeyCode::Down | KeyCode::Char('j') => Some(scroll.saturating_add(1)),
        KeyCode::PageUp => Some(scroll.saturating_sub(15)),
        KeyCode::PageDown => Some(scroll.saturating_add(15)),
        KeyCode::Home => Some(0),
        KeyCode::End => Some(line_count.saturating_sub(1) as u16),
        _ => Some(scroll),
    }
}

fn move_selection(state: &mut TableState, len: usize, code: KeyCode) {
    if len == 0 {
        state.select(None);
        return;
    }
    let current = state.selected().unwrap_or(0).min(len - 1);
    let next = match code {
        KeyCode::Up | KeyCode::Char('k') => current.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => (current + 1).min(len - 1),
        KeyCode::PageUp => current.saturating_sub(10),
        KeyCode::PageDown => (current + 10).min(len - 1),
        KeyCode::Home => 0,
        KeyCode::End => len - 1,
        _ => current,
    };
    state.select(Some(next));
}
