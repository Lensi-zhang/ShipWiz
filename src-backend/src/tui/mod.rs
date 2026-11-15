mod ui;

use crate::compose::{self, ComposeDeployment, ComposeProject};
use crate::config::{
    self, FleetState, NodeConnection, NodeInput, NodeStatus, NodeType, RegistrySource, ShipNode,
    SshAuthMethod,
};
use crate::credential;
use crate::docker::{self, container, image};
use crate::error::AppError;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::TableState;
use ratatui::Terminal;
use std::io;
use std::time::Duration;
use tokio::runtime::Handle;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
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

struct FormField {
    label: &'static str,
    value: String,
}

struct Form {
    title: String,
    kind: FormKind,
    fields: Vec<FormField>,
    focus: usize,
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

    fn value(&self, index: usize) -> String {
        self.fields
            .get(index)
            .map(|field| field.value.trim().to_string())
            .unwrap_or_default()
    }
}

#[derive(Clone)]
enum FormKind {
    CreateContainer,
    Node { id: Option<String> },
    AddCompose,
    AddRegistry,
    PullImage,
    SearchImage,
    ExportImage { reference: String },
    ImportImage,
    Exec { id: String, name: String },
}

enum Action {
    ContainerRemove { id: String, name: String },
    ImageRemove { id: String, name: String },
    ComposeUp { id: String },
    ComposeDown { id: String },
    ComposeRemove { id: String, name: String },
    NodeRemove { id: String, name: String },
    RegistryRemove { id: String, name: String },
    RegistryDefault { id: String, name: String },
}

enum Overlay {
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
    Search {
        term: String,
        results: Vec<image::ImageSearchResult>,
        state: TableState,
    },
}

pub struct App {
    handle: Handle,
    tab: Tab,
    state: FleetState,
    registries: Vec<RegistrySource>,
    containers: Vec<container::ContainerInfo>,
    images: Vec<image::ImageInfo>,
    projects: Vec<ComposeProject>,
    deployments: Vec<ComposeDeployment>,
    container_state: TableState,
    image_state: TableState,
    project_state: TableState,
    node_state: TableState,
    registry_state: TableState,
    overlay: Option<Overlay>,
    status: String,
    should_quit: bool,
}

impl App {
    fn new(handle: Handle) -> Self {
        let mut app = Self {
            handle,
            tab: Tab::Containers,
            state: FleetState {
                nodes: Vec::new(),
                current_node_id: None,
            },
            registries: Vec::new(),
            containers: Vec::new(),
            images: Vec::new(),
            projects: Vec::new(),
            deployments: Vec::new(),
            container_state: TableState::default(),
            image_state: TableState::default(),
            project_state: TableState::default(),
            node_state: TableState::default(),
            registry_state: TableState::default(),
            overlay: None,
            status: "正在接入舰队…".to_string(),
            should_quit: false,
        };
        app.refresh_all();
        app
    }

    fn refresh_all(&mut self) {
        self.load_state();
        self.load_registries();
        self.reload_containers();
        self.reload_images();
        self.reload_compose();
    }

    fn load_state(&mut self) {
        match config::load_state() {
            Ok(state) => {
                self.state = state;
                let current = self
                    .state
                    .current_node_id
                    .as_deref()
                    .and_then(|id| self.state.nodes.iter().position(|node| node.id == id));
                self.node_state
                    .select(current.or_else(|| (!self.state.nodes.is_empty()).then_some(0)));
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn load_registries(&mut self) {
        match config::load_registries() {
            Ok(registries) => {
                self.registries = registries;
                clamp(&mut self.registry_state, self.registries.len());
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn reload_containers(&mut self) {
        let Some(node) = self.active_node() else {
            self.containers.clear();
            self.container_state.select(None);
            return;
        };
        match self.handle.block_on(container::list_for_node(&node)) {
            Ok(containers) => {
                self.containers = containers;
                clamp(&mut self.container_state, self.containers.len());
            }
            Err(error) => {
                self.containers.clear();
                self.container_state.select(None);
                self.status = error.message();
            }
        }
    }

    fn reload_images(&mut self) {
        let Some(node) = self.active_node() else {
            self.images.clear();
            self.image_state.select(None);
            return;
        };
        match self.handle.block_on(image::list_for_node(&node)) {
            Ok(images) => {
                self.images = images;
                clamp(&mut self.image_state, self.images.len());
            }
            Err(error) => {
                self.images.clear();
                self.image_state.select(None);
                self.status = error.message();
            }
        }
    }

    fn reload_compose(&mut self) {
        match compose::load_projects() {
            Ok(mut projects) => {
                projects.sort_by(|left, right| right.updated_time.cmp(&left.updated_time));
                self.projects = projects;
                clamp(&mut self.project_state, self.projects.len());
            }
            Err(error) => self.status = error.message(),
        }
        let Some(node) = self.active_node() else {
            self.deployments.clear();
            return;
        };
        match self.handle.block_on(compose::list_for_node(&node)) {
            Ok(deployments) => self.deployments = deployments,
            Err(_) => self.deployments.clear(),
        }
    }

    fn active_node(&self) -> Option<ShipNode> {
        let id = self.state.current_node_id.as_deref()?;
        self.state.nodes.iter().find(|node| node.id == id).cloned()
    }

    fn on_key(&mut self, key: KeyEvent) {
        if self.overlay.is_some() {
            let overlay = self.overlay.take().expect("overlay 存在");
            self.handle_overlay(overlay, key);
            return;
        }

        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('?') => {
                self.tab = Tab::Help;
                return;
            }
            KeyCode::F(5) => {
                self.refresh_all();
                self.status = "已刷新".to_string();
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
                let next = match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.overlay = None;
                        return;
                    }
                    KeyCode::Up | KeyCode::Char('k') => scroll.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => scroll.saturating_add(1),
                    KeyCode::PageUp => scroll.saturating_sub(15),
                    KeyCode::PageDown => scroll.saturating_add(15),
                    KeyCode::Home => 0,
                    KeyCode::End => lines.len().saturating_sub(1) as u16,
                    _ => scroll,
                };
                self.overlay = Some(Overlay::Viewer {
                    title,
                    lines,
                    scroll: next,
                });
            }
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

    fn start_container(&mut self, id: String, name: String) {
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        match self.handle.block_on(container::start_for_node(&node, &id)) {
            Ok(_) => {
                self.status = format!("集装箱 {name} 已启航");
                self.reload_containers();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn stop_container(&mut self, id: String, name: String) {
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        match self.handle.block_on(container::stop_for_node(&node, &id)) {
            Ok(_) => {
                self.status = format!("集装箱 {name} 已停泊");
                self.reload_containers();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn restart_container(&mut self, id: String, name: String) {
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        match self
            .handle
            .block_on(container::restart_for_node(&node, &id))
        {
            Ok(_) => {
                self.status = format!("集装箱 {name} 已重启");
                self.reload_containers();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn remove_container(&mut self, id: String, name: String) {
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        match self
            .handle
            .block_on(container::remove_for_node(&node, &id, true))
        {
            Ok(_) => {
                self.status = format!("集装箱 {name} 已撤收");
                self.reload_containers();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn show_container_detail(&mut self) {
        let Some(index) = self
            .container_state
            .selected()
            .filter(|index| *index < self.containers.len())
        else {
            return;
        };
        let entry = &self.containers[index];
        let lines = vec![
            format!("名称：{}", entry.name),
            format!("短 ID：{}", entry.short_id),
            format!("完整 ID：{}", entry.id),
            format!("图纸：{}", entry.image),
            format!("图纸 ID：{}", entry.image_id),
            format!("状态：{}", entry.state),
            format!("详情：{}", entry.status),
            format!("端口：{}", format_ports(&entry.ports)),
            format!("CPU：{:.2}%", entry.cpu_percent),
            format!(
                "内存：{} / {}",
                human_size(entry.memory_usage as i64),
                human_size(entry.memory_limit as i64)
            ),
            format!(
                "创建时间：{}",
                format_timestamp(entry.created.max(0) as u64 * 1000)
            ),
        ];
        self.overlay = Some(Overlay::Viewer {
            title: format!("集装箱详情 · {}", entry.name),
            lines,
            scroll: 0,
        });
    }

    fn show_logs(&mut self, id: String, name: String) {
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        match self
            .handle
            .block_on(container::logs_for_node(&node, &id, 200))
        {
            Ok(output) => {
                self.overlay = Some(Overlay::Viewer {
                    title: format!("{name} 日志（最近 200 行）"),
                    lines: split_lines(&output),
                    scroll: 0,
                });
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn show_compose_logs(&mut self, name: String) {
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        match self
            .handle
            .block_on(compose::logs_for_node(&node, &name, 200))
        {
            Ok(output) => {
                self.overlay = Some(Overlay::Viewer {
                    title: format!("编队 {name} 日志（最近 200 行）"),
                    lines: split_lines(&output),
                    scroll: 0,
                });
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn remove_image(&mut self, id: String, name: String) {
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        match self
            .handle
            .block_on(image::remove_for_node(&node, &id, false))
        {
            Ok(_) => {
                self.status = format!("图纸 {name} 已移除");
                self.reload_images();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn pull_image(&mut self, reference: String) {
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        match self
            .handle
            .block_on(image::pull_for_node(&node, &reference, None))
        {
            Ok(result) => {
                self.status = format!("图纸 {} 已入库", result.reference);
                self.overlay = Some(Overlay::Viewer {
                    title: format!("采购 {}", result.reference),
                    lines: split_lines(&result.output),
                    scroll: 0,
                });
                self.reload_images();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn export_image(&mut self, reference: String, destination: String) {
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        match self
            .handle
            .block_on(image::export_for_node(&node, &reference, &destination))
        {
            Ok(_) => self.status = format!("已导出 {reference} 到 {destination}"),
            Err(error) => self.status = error.message(),
        }
    }

    fn import_image(&mut self, source: String) {
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        match self.handle.block_on(image::import_for_node(&node, &source)) {
            Ok(result) => {
                self.status = format!("已导入 {}", result.reference);
                self.reload_images();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn run_search(&mut self, term: String) {
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        match self.handle.block_on(image::search_for_node(&node, &term)) {
            Ok(results) => {
                self.status = format!("检索到 {} 个图纸", results.len());
                let mut state = TableState::default();
                if !results.is_empty() {
                    state.select(Some(0));
                }
                self.overlay = Some(Overlay::Search {
                    term,
                    results,
                    state,
                });
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn compose_action(&mut self, id: String, up: bool) {
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        let project = match compose::find_project(&id) {
            Ok(project) => project,
            Err(error) => {
                self.status = error.message();
                return;
            }
        };
        let result = if up {
            self.handle.block_on(compose::up_for_node(&node, &project))
        } else {
            self.handle
                .block_on(compose::down_for_node(&node, &project))
        };
        match result {
            Ok(result) => {
                self.status = format!(
                    "编队 {} 已{}",
                    project.name,
                    if up { "启动" } else { "停止" }
                );
                self.overlay = Some(Overlay::Viewer {
                    title: format!("编队 {}", project.name),
                    lines: split_lines(&result.output),
                    scroll: 0,
                });
                self.reload_compose();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn remove_project(&mut self, id: String, name: String) {
        match compose::remove_project(&id) {
            Ok(_) => {
                self.status = format!("编队 {name} 已移除");
                self.reload_compose();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn use_node(&mut self) {
        let Some(index) = self
            .node_state
            .selected()
            .filter(|index| *index < self.state.nodes.len())
        else {
            return;
        };
        let id = self.state.nodes[index].id.clone();
        let name = self.state.nodes[index].name.clone();
        self.state.current_node_id = Some(id);
        match config::write_state(&self.state) {
            Ok(()) => {
                self.status = format!("当前节点已切换为 {name}");
                self.reload_containers();
                self.reload_images();
                self.reload_compose();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn show_node_overview(&mut self) {
        let Some(index) = self
            .node_state
            .selected()
            .filter(|index| *index < self.state.nodes.len())
        else {
            return;
        };
        let node = self.state.nodes[index].clone();
        match self.handle.block_on(container::overview_for_node(&node)) {
            Ok((total, running, images)) => {
                self.overlay = Some(Overlay::Viewer {
                    title: format!("{} 概览", node.name),
                    lines: vec![
                        format!("节点：{}", node.name),
                        format!("地址：{}", address(&node)),
                        format!("集装箱：{total}（运行 {running}）"),
                        format!("图纸：{images}"),
                    ],
                    scroll: 0,
                });
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn test_node(&mut self) {
        let Some(index) = self
            .node_state
            .selected()
            .filter(|index| *index < self.state.nodes.len())
        else {
            return;
        };
        let node = self.state.nodes[index].clone();
        docker::disconnect_node(&node.id);
        match self.handle.block_on(docker::check_connection(&node)) {
            Ok(()) => {
                self.state.nodes[index].status = NodeStatus::Online;
                self.state.nodes[index].last_success_time = Some(crate::utils::timestamp_ms());
                let name = node.name.clone();
                if let Err(error) = config::write_state(&self.state) {
                    self.status = error.message();
                } else {
                    self.status = format!("{name} 连接成功");
                }
            }
            Err(error) => {
                self.state.nodes[index].status = NodeStatus::Offline;
                let _ = config::write_state(&self.state);
                self.status = error.message();
            }
        }
    }

    fn remove_node(&mut self, id: String, name: String) {
        let mut state = match config::load_state() {
            Ok(state) => state,
            Err(error) => {
                self.status = error.message();
                return;
            }
        };
        let Some(existing) = state.nodes.iter().find(|node| node.id == id).cloned() else {
            self.status = "目标舰船不存在".to_string();
            return;
        };
        state.nodes.retain(|node| node.id != id);
        if state.current_node_id.as_deref() == Some(id.as_str()) {
            state.current_node_id = state.nodes.first().map(|node| node.id.clone());
        }
        if let Err(error) = config::write_state(&state) {
            self.status = error.message();
            return;
        }
        if let Some(credential_id) = existing.credential_id {
            let _ = credential::delete(&credential_id);
        }
        docker::disconnect_node(&id);
        self.status = format!("舰船 {name} 已移出舰队");
        self.load_state();
        self.reload_containers();
        self.reload_images();
        self.reload_compose();
    }

    fn remove_registry(&mut self, id: String, name: String) {
        let mut registries = match config::load_registries() {
            Ok(registries) => registries,
            Err(error) => {
                self.status = error.message();
                return;
            }
        };
        registries.retain(|registry| registry.id != id);
        if let Err(error) = config::ensure_default_registry(&mut registries, None) {
            self.status = error.message();
            return;
        }
        match config::write_registries(&registries) {
            Ok(()) => {
                self.status = format!("补给港 {name} 已移除");
                self.load_registries();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn set_default_registry(&mut self, id: String, name: String) {
        let mut registries = match config::load_registries() {
            Ok(registries) => registries,
            Err(error) => {
                self.status = error.message();
                return;
            }
        };
        for registry in &mut registries {
            registry.is_default = registry.id == id;
        }
        match config::write_registries(&registries) {
            Ok(()) => {
                self.status = format!("默认补给港已切换为 {name}");
                self.load_registries();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn submit_form(&mut self, form: Form) {
        match form.kind.clone() {
            FormKind::CreateContainer => self.create_container(form),
            FormKind::Node { id } => match id {
                Some(id) => self.edit_node(form, id),
                None => self.add_node(form),
            },
            FormKind::AddCompose => self.add_compose(form),
            FormKind::AddRegistry => self.add_registry(form),
            FormKind::PullImage => self.pull_image(form.value(0)),
            FormKind::SearchImage => self.run_search(form.value(0)),
            FormKind::ExportImage { reference } => self.export_image(reference, form.value(0)),
            FormKind::ImportImage => self.import_image(form.value(0)),
            FormKind::Exec { id, name } => self.run_exec(form, id, name),
        }
    }

    fn create_container(&mut self, form: Form) {
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        let env = form
            .value(3)
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let ports = match parse_ports(&form.value(4)) {
            Ok(ports) => ports,
            Err(error) => {
                self.status = error.message();
                return;
            }
        };
        let input = container::CreateContainerInput {
            name: form.value(0),
            image: form.value(1),
            command: form.value(2),
            env,
            ports,
            volumes: Vec::new(),
            restart_policy: "no".to_string(),
            network: String::new(),
            auto_remove: false,
            start_after_create: true,
        };
        match self
            .handle
            .block_on(container::create_for_node(&node, input))
        {
            Ok(id) => {
                self.status = format!("已创建集装箱 {}", short(&id));
                self.reload_containers();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn add_node(&mut self, form: Form) {
        let input = match build_node_input(&form) {
            Ok(input) => input,
            Err(error) => {
                self.status = error.message();
                return;
            }
        };
        let prepared = match config::prepare_node(input, None) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.status = error.message();
                return;
            }
        };
        if let Some((id, stored)) = &prepared.save_credential {
            if let Err(error) = credential::save(id, stored) {
                self.status = error.message();
                return;
            }
        }
        let mut state = match config::load_state() {
            Ok(state) => state,
            Err(error) => {
                self.status = error.message();
                return;
            }
        };
        let name = prepared.node.name.clone();
        state.nodes.push(prepared.node);
        match config::write_state(&state) {
            Ok(()) => {
                self.status = format!("已新增节点 {name}");
                self.load_state();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn edit_node(&mut self, form: Form, id: String) {
        let mut state = match config::load_state() {
            Ok(state) => state,
            Err(error) => {
                self.status = error.message();
                return;
            }
        };
        let Some(existing) = state.nodes.iter().find(|node| node.id == id).cloned() else {
            self.status = "目标舰船不存在".to_string();
            return;
        };
        let input = match build_node_input(&form) {
            Ok(input) => input,
            Err(error) => {
                self.status = error.message();
                return;
            }
        };
        let prepared = match config::prepare_node(input, Some(&existing)) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.status = error.message();
                return;
            }
        };
        if let Some((credential_id, stored)) = &prepared.save_credential {
            if let Err(error) = credential::save(credential_id, stored) {
                self.status = error.message();
                return;
            }
        }
        let name = prepared.node.name.clone();
        match config::find_node_mut(&mut state, &id) {
            Ok(slot) => *slot = prepared.node,
            Err(error) => {
                self.status = error.message();
                return;
            }
        }
        if let Err(error) = config::write_state(&state) {
            self.status = error.message();
            return;
        }
        docker::disconnect_node(&id);
        for credential_id in prepared.delete_credentials {
            let _ = credential::delete(&credential_id);
        }
        self.status = format!("节点 {name} 已更新");
        self.load_state();
    }

    fn add_compose(&mut self, form: Form) {
        let name = form.value(0);
        let path = form.value(1);
        let yaml = match std::fs::read_to_string(&path) {
            Ok(yaml) => yaml,
            Err(error) => {
                self.status = format!("无法读取编队文件 {path}：{error}");
                return;
            }
        };
        match compose::save_project(&name, &yaml, None) {
            Ok(project) => {
                self.status = format!("已保存编队 {}", project.name);
                self.reload_compose();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn add_registry(&mut self, form: Form) {
        let (name, url) = match config::normalize_registry(form.value(0), form.value(1)) {
            Ok(pair) => pair,
            Err(error) => {
                self.status = error.message();
                return;
            }
        };
        let mut registries = match config::load_registries() {
            Ok(registries) => registries,
            Err(error) => {
                self.status = error.message();
                return;
            }
        };
        let id = Uuid::new_v4().to_string();
        registries.push(RegistrySource {
            id,
            name: name.clone(),
            url,
            is_default: false,
        });
        if let Err(error) = config::ensure_default_registry(&mut registries, None) {
            self.status = error.message();
            return;
        }
        match config::write_registries(&registries) {
            Ok(()) => {
                self.status = format!("已新增补给港 {name}");
                self.load_registries();
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn run_exec(&mut self, form: Form, id: String, name: String) {
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        let command = form.value(0);
        match self
            .handle
            .block_on(container::exec_for_node(&node, &id, &command))
        {
            Ok(result) => {
                let mut lines = split_lines(&result.output);
                if let Some(code) = result.exit_code {
                    lines.push(format!("退出码：{code}"));
                }
                self.overlay = Some(Overlay::Viewer {
                    title: format!("{name} · {command}"),
                    lines,
                    scroll: 0,
                });
            }
            Err(error) => self.status = error.message(),
        }
    }
}

fn build_node_input(form: &Form) -> Result<NodeInput, AppError> {
    let name = form.value(0);
    if name.is_empty() {
        return Err(AppError::validation("舰船名称不能为空。"));
    }
    let node_type = match form.value(2).to_ascii_lowercase().as_str() {
        "remote" => NodeType::Remote,
        _ => NodeType::Local,
    };
    let mut connection = match form.value(3).to_ascii_lowercase().as_str() {
        "ssh" => NodeConnection::Ssh,
        "local" => NodeConnection::Local,
        _ => NodeConnection::Tls,
    };
    if node_type == NodeType::Local {
        connection = NodeConnection::Local;
    } else if connection == NodeConnection::Local {
        connection = NodeConnection::Tls;
    }
    let raw_port = form.value(5);
    let port = if raw_port.is_empty() {
        match connection {
            NodeConnection::Tls => 2376,
            NodeConnection::Ssh => 22,
            NodeConnection::Local => 0,
        }
    } else {
        raw_port
            .parse::<u16>()
            .map_err(|_| AppError::validation("端口必须是 1–65535 之间的数字。"))?
    };
    Ok(NodeInput {
        name,
        alias: form.value(1),
        node_type,
        connection,
        host: form.value(4),
        port,
        tls: connection == NodeConnection::Tls,
        cert_path: nonempty(form.value(6)),
        username: nonempty(form.value(7)),
        auth_method: (connection == NodeConnection::Ssh).then_some(SshAuthMethod::Password),
        private_key_path: None,
        remote_socket: None,
        password: nonempty(form.value(8)),
        private_key: None,
        passphrase: None,
        clear_credential: false,
    })
}

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn parse_ports(raw: &str) -> Result<Vec<container::PortInput>, AppError> {
    let mut ports = Vec::new();
    for item in raw
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let (mapping, protocol) = match item.split_once('/') {
            Some((mapping, protocol)) => (mapping, protocol.to_string()),
            None => (item, "tcp".to_string()),
        };
        let (host_port, container_port) = mapping
            .split_once(':')
            .ok_or_else(|| AppError::input(format!("端口映射“{item}”应为 host:container。")))?;
        ports.push(container::PortInput {
            host_port: host_port
                .parse()
                .map_err(|_| AppError::input(format!("主机端口“{host_port}”无效。")))?,
            container_port: container_port
                .parse()
                .map_err(|_| AppError::input(format!("集装箱端口“{container_port}”无效。")))?,
            protocol,
            host_ip: String::new(),
        });
    }
    Ok(ports)
}

fn split_lines(text: &str) -> Vec<String> {
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

fn short(id: &str) -> String {
    id.chars().take(12).collect()
}

fn clamp(state: &mut TableState, len: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    match state.selected() {
        Some(index) if index < len => {}
        Some(_) => state.select(Some(len - 1)),
        None => state.select(Some(0)),
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

fn address(node: &ShipNode) -> String {
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

fn format_ports(ports: &[container::ContainerPort]) -> String {
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

fn human_size(bytes: i64) -> String {
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

fn format_timestamp(ms: u64) -> String {
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

pub fn run() -> i32 {
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
    match run_loop(runtime.handle().clone()) {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("{message}");
            1
        }
    }
}

fn run_loop(handle: Handle) -> Result<(), String> {
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
    loop {
        terminal
            .draw(|frame| ui::draw(frame, app))
            .map_err(|error| error.to_string())?;
        if event::poll(Duration::from_millis(250)).map_err(|error| error.to_string())? {
            if let Event::Key(key) = event::read().map_err(|error| error.to_string())? {
                if key.kind == KeyEventKind::Press {
                    app.on_key(key);
                }
            }
        }
        if app.should_quit {
            return Ok(());
        }
    }
}
