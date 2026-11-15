use super::input::{Form, FormKind, Overlay};
use super::{address, format_ports, format_timestamp, human_size, split_lines, Tab};
use crate::compose::{self, ComposeDeployment, ComposeProject};
use crate::config::{
    self, FleetState, NodeConnection, NodeInput, NodeStatus, NodeType, RegistrySource, ShipNode,
    SshAuthMethod,
};
use crate::credential;
use crate::docker::{
    self, container,
    image::{self, ImageOperationResult, ImagePullProgress},
};
use crate::error::AppError;
use crate::template::{self, ComposeTemplate};
use crate::update::{self, UpdateInfo};
use ratatui::widgets::TableState;
use std::collections::VecDeque;
use std::sync::mpsc;
use tokio::runtime::Handle;
use uuid::Uuid;

struct UpdateMessage {
    manual: bool,
    result: Result<Option<UpdateInfo>, String>,
}

pub(super) struct PullOperation {
    pub(super) id: u64,
    pub(super) reference: String,
    pub(super) layers: Vec<ImagePullProgress>,
}

impl PullOperation {
    fn apply(&mut self, progress: ImagePullProgress) {
        if let Some(layer) = self
            .layers
            .iter_mut()
            .find(|layer| layer.layer_id == progress.layer_id)
        {
            *layer = progress;
        } else {
            self.layers.push(progress);
        }
    }
}

enum PullMessage {
    Progress {
        operation_id: u64,
        progress: ImagePullProgress,
    },
    Finished {
        operation_id: u64,
        result: Result<ImageOperationResult, String>,
    },
}

struct ComposeMessage {
    operation_id: u64,
    up: bool,
    name: String,
    result: Result<compose::ComposeCommandResult, String>,
}

pub struct App {
    handle: Handle,
    pub(super) tab: Tab,
    pub(super) state: FleetState,
    pub(super) registries: Vec<RegistrySource>,
    pub(super) containers: Vec<container::ContainerInfo>,
    pub(super) images: Vec<image::ImageInfo>,
    pub(super) projects: Vec<ComposeProject>,
    pub(super) deployments: Vec<ComposeDeployment>,
    pub(super) container_state: TableState,
    pub(super) image_state: TableState,
    pub(super) project_state: TableState,
    pub(super) node_state: TableState,
    pub(super) registry_state: TableState,
    pub(super) overlay: Option<Overlay>,
    pub(super) status: String,
    pub(super) command_input: String,
    pub(super) command_focused: bool,
    update_sender: mpsc::Sender<UpdateMessage>,
    update_receiver: mpsc::Receiver<UpdateMessage>,
    update_busy: bool,
    update_manual_pending: bool,
    pull_sender: mpsc::Sender<PullMessage>,
    pull_receiver: mpsc::Receiver<PullMessage>,
    pub(super) pull_operation: Option<PullOperation>,
    compose_sender: mpsc::Sender<ComposeMessage>,
    compose_receiver: mpsc::Receiver<ComposeMessage>,
    compose_operation: Option<u64>,
    next_operation_id: u64,
    pending_viewers: VecDeque<Overlay>,
    dirty: bool,
    pub(super) should_quit: bool,
}

impl App {
    pub(super) fn new(handle: Handle) -> Self {
        let (update_sender, update_receiver) = mpsc::channel();
        let (pull_sender, pull_receiver) = mpsc::channel();
        let (compose_sender, compose_receiver) = mpsc::channel();
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
            command_input: String::new(),
            command_focused: false,
            update_sender,
            update_receiver,
            update_busy: false,
            update_manual_pending: false,
            pull_sender,
            pull_receiver,
            pull_operation: None,
            compose_sender,
            compose_receiver,
            compose_operation: None,
            next_operation_id: 1,
            pending_viewers: VecDeque::new(),
            dirty: true,
            should_quit: false,
        };
        app.start_update_check(false);
        app.refresh_all();
        app
    }

    /// 后台检查 GitHub Release，不阻塞主界面；手动检查会给出错误提示
    pub(super) fn start_update_check(&mut self, manual: bool) {
        if self.update_busy {
            if manual {
                self.update_manual_pending = true;
                self.status = "当前检查结束后将强制检查更新…".to_string();
            }
            return;
        }
        self.update_busy = true;
        if manual {
            self.status = "正在检查更新…".to_string();
        }
        let sender = self.update_sender.clone();
        self.handle.spawn(async move {
            let result = update::check(manual)
                .await
                .map_err(|error| error.message().replace('\n', "  "));
            let _ = sender.send(UpdateMessage { manual, result });
        });
    }

    fn poll_update_result(&mut self) {
        while let Ok(message) = self.update_receiver.try_recv() {
            self.update_busy = false;
            self.dirty = true;
            if self.update_manual_pending {
                self.update_manual_pending = false;
                self.start_update_check(true);
                continue;
            }
            match message.result {
                Ok(Some(info)) => {
                    self.status = format!("发现新版本 v{} · 按 U 查看更新说明", info.latest);
                    if message.manual {
                        self.show_update_viewer(info);
                    }
                }
                Ok(None) => {
                    if message.manual {
                        self.status = format!("已是最新版本 v{}", update::current_version());
                    }
                }
                Err(error) => {
                    // 启动时的自动检查失败保持静默，避免每次启动都报错
                    if message.manual {
                        self.status = format!("检查更新失败：{error}");
                    }
                }
            }
        }
    }

    fn show_update_viewer(&mut self, info: UpdateInfo) {
        let mut lines = vec![
            format!("当前版本：v{}", info.current),
            format!("最新版本：v{}", info.latest),
        ];
        if !info.published_at.is_empty() {
            lines.push(format!("发布时间：{}", info.published_at));
        }
        if let Ok(Some(skipped)) = update::skipped_version() {
            if skipped == info.latest {
                lines.push("提醒状态：此版本已跳过；按 C 可恢复全部更新提醒".to_string());
            }
        }
        lines.push(String::new());
        lines.push("更新说明".to_string());
        if info.notes.is_empty() {
            lines.push("（该版本没有填写更新说明）".to_string());
        } else {
            lines.extend(info.notes.lines().take(60).map(str::to_string));
        }
        lines.push(String::new());
        lines.push(format!("下载地址：{}", info.url));
        let viewer = Overlay::UpdateViewer {
            title: format!("发现新版本 · v{}", info.latest),
            lines,
            scroll: 0,
            info,
        };
        if self.overlay.is_none() {
            self.overlay = Some(viewer);
        } else {
            self.pending_viewers.push_back(viewer);
        }
        self.dirty = true;
    }

    pub(super) fn skip_current_update(&mut self, info: UpdateInfo) {
        match update::skip_version(info.latest.clone()) {
            Ok(()) => {
                self.status = format!("已跳过 v{} 的更新提醒；按 U 可手动查看", info.latest);
                self.overlay = None;
            }
            Err(error) => {
                self.status = format!("跳过更新提醒失败：{}", error.message());
                self.show_update_viewer(info);
            }
        }
    }

    pub(super) fn restore_update_notifications(&mut self, info: UpdateInfo) {
        match update::clear_skipped() {
            Ok(()) => self.status = "已恢复全部更新提醒".to_string(),
            Err(error) => self.status = format!("恢复更新提醒失败：{}", error.message()),
        }
        self.show_update_viewer(info);
    }

    pub(super) fn poll_background(&mut self) {
        self.poll_update_result();
        self.poll_pull_messages();
        self.poll_compose_messages();
        self.show_pending_viewer();
    }

    pub(super) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub(super) fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub(super) fn can_auto_refresh(&self) -> bool {
        auto_refresh_allowed(
            self.tab,
            self.overlay.is_some(),
            self.pull_operation.is_some(),
            self.compose_operation.is_some(),
        )
    }

    pub(super) fn refresh_current_page(&mut self) {
        match self.tab {
            Tab::Containers => self.reload_containers(),
            Tab::Images => self.reload_images(),
            Tab::Compose => self.reload_compose(),
            _ => return,
        }
        self.dirty = true;
    }

    pub(super) fn compose_busy(&self) -> bool {
        self.compose_operation.is_some()
    }

    fn allocate_operation_id(&mut self) -> u64 {
        let id = self.next_operation_id;
        self.next_operation_id = self.next_operation_id.wrapping_add(1);
        id
    }

    fn present_viewer(&mut self, title: String, lines: Vec<String>) {
        let viewer = Overlay::Viewer {
            title,
            lines,
            scroll: 0,
        };
        if self.overlay.is_none() {
            self.overlay = Some(viewer);
        } else {
            self.pending_viewers.push_back(viewer);
        }
        self.dirty = true;
    }

    pub(super) fn show_pending_viewer(&mut self) {
        if self.overlay.is_none() {
            if let Some(viewer) = self.pending_viewers.pop_front() {
                self.overlay = Some(viewer);
                self.dirty = true;
            }
        }
    }

    pub(super) fn refresh_all(&mut self) {
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
                projects.sort_by_key(|project| std::cmp::Reverse(project.updated_time));
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

    pub(super) fn active_node(&self) -> Option<ShipNode> {
        let id = self.state.current_node_id.as_deref()?;
        self.state.nodes.iter().find(|node| node.id == id).cloned()
    }

    pub(super) fn start_container(&mut self, id: String, name: String) {
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

    pub(super) fn stop_container(&mut self, id: String, name: String) {
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

    pub(super) fn restart_container(&mut self, id: String, name: String) {
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

    pub(super) fn remove_container(&mut self, id: String, name: String) {
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

    pub(super) fn show_container_detail(&mut self) {
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

    pub(super) fn show_logs(&mut self, id: String, name: String) {
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

    pub(super) fn show_compose_logs(&mut self, name: String) {
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

    pub(super) fn remove_image(&mut self, id: String, name: String) {
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

    pub(super) fn pull_image(&mut self, reference: String) {
        if self.pull_operation.is_some() {
            self.status = "已有图纸采购任务正在进行，请等待完成".to_string();
            return;
        }
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        let registry_id = self
            .registries
            .iter()
            .find(|registry| registry.is_default)
            .map(|registry| registry.id.clone());
        let operation_id = self.allocate_operation_id();
        self.pull_operation = Some(PullOperation {
            id: operation_id,
            reference: reference.clone(),
            layers: Vec::new(),
        });
        self.status = format!("正在采购图纸 {reference}…");
        self.dirty = true;

        let sender = self.pull_sender.clone();
        let progress_sender = sender.clone();
        self.handle.spawn(async move {
            let result = image::pull_for_node_with_progress(
                &node,
                &reference,
                registry_id.as_deref(),
                |progress| {
                    let _ = progress_sender.send(PullMessage::Progress {
                        operation_id,
                        progress,
                    });
                },
            )
            .await
            .map_err(|error| error.message());
            let _ = sender.send(PullMessage::Finished {
                operation_id,
                result,
            });
        });
    }

    fn poll_pull_messages(&mut self) {
        while let Ok(message) = self.pull_receiver.try_recv() {
            let operation_id = match &message {
                PullMessage::Progress { operation_id, .. }
                | PullMessage::Finished { operation_id, .. } => *operation_id,
            };
            if self.pull_operation.as_ref().map(|operation| operation.id) != Some(operation_id) {
                continue;
            }
            self.dirty = true;
            match message {
                PullMessage::Progress { progress, .. } => {
                    if let Some(operation) = self.pull_operation.as_mut() {
                        operation.apply(progress);
                    }
                }
                PullMessage::Finished { result, .. } => {
                    self.pull_operation = None;
                    self.reload_images();
                    match result {
                        Ok(result) => {
                            self.status = format!("图纸 {} 已入库", result.reference);
                            self.present_viewer(
                                format!("采购 {}", result.reference),
                                split_lines(&result.output),
                            );
                        }
                        Err(message) => {
                            self.status = format!("图纸采购失败：{message}");
                            self.present_viewer("图纸采购失败".to_string(), split_lines(&message));
                        }
                    }
                }
            }
        }
    }

    pub(super) fn export_image(&mut self, reference: String, destination: String) {
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

    pub(super) fn import_image(&mut self, source: String) {
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

    pub(super) fn run_search(&mut self, term: String) {
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

    pub(super) fn compose_action(&mut self, id: String, up: bool) {
        if self.compose_busy() {
            self.status = "已有编队操作正在进行，请等待完成".to_string();
            return;
        }
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
        self.start_compose_operation(node, project, up);
    }

    pub(super) fn open_compose_templates(&mut self) {
        if self.compose_busy() {
            self.status = "已有编队操作正在进行，请等待完成".to_string();
            return;
        }
        match template::list() {
            Ok(templates) => {
                let mut state = TableState::default();
                state.select((!templates.is_empty()).then_some(0));
                self.overlay = Some(Overlay::Templates { templates, state });
            }
            Err(error) => self.status = error.message(),
        }
    }

    pub(super) fn create_compose_from_template(&mut self, name: String, template: ComposeTemplate) {
        if self.compose_busy() {
            self.status = "已有编队操作正在进行，请等待完成".to_string();
            return;
        }
        let Some(node) = self.active_node() else {
            self.status = "尚未选择节点".to_string();
            return;
        };
        match compose::save_project(&name, &template.yaml, None) {
            Ok(project) => {
                self.reload_compose();
                self.start_compose_operation(node, project, true);
            }
            Err(error) => self.status = error.message(),
        }
    }

    fn start_compose_operation(&mut self, node: ShipNode, project: ComposeProject, up: bool) {
        let operation_id = self.allocate_operation_id();
        let name = project.name.clone();
        self.compose_operation = Some(operation_id);
        self.status = format!("正在{}编队 {name}…", if up { "启动" } else { "停止" });
        self.dirty = true;
        let sender = self.compose_sender.clone();
        self.handle.spawn(async move {
            let result = if up {
                compose::up_for_node(&node, &project).await
            } else {
                compose::down_for_node(&node, &project).await
            }
            .map_err(|error| error.message());
            let _ = sender.send(ComposeMessage {
                operation_id,
                up,
                name,
                result,
            });
        });
    }

    fn poll_compose_messages(&mut self) {
        while let Ok(message) = self.compose_receiver.try_recv() {
            if self.compose_operation != Some(message.operation_id) {
                continue;
            }
            self.compose_operation = None;
            self.dirty = true;
            self.reload_compose();
            let action = if message.up { "启动" } else { "停止" };
            match message.result {
                Ok(result) => {
                    self.status = format!("编队 {} 已{action}", message.name);
                    self.present_viewer(
                        format!("编队 {} · {action}", message.name),
                        split_lines(&result.output),
                    );
                }
                Err(error) => {
                    self.status = format!("编队 {} {action}失败：{error}", message.name);
                    self.present_viewer(
                        format!("编队 {} · {action}失败", message.name),
                        split_lines(&error),
                    );
                }
            }
        }
    }

    pub(super) fn remove_project(&mut self, id: String, name: String) {
        match compose::remove_project(&id) {
            Ok(_) => {
                self.status = format!("编队 {name} 已移除");
                self.reload_compose();
            }
            Err(error) => self.status = error.message(),
        }
    }

    pub(super) fn use_node(&mut self) {
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

    pub(super) fn show_node_overview(&mut self) {
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

    pub(super) fn test_node(&mut self) {
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

    pub(super) fn remove_node(&mut self, id: String, name: String) {
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

    pub(super) fn remove_registry(&mut self, id: String, name: String) {
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

    pub(super) fn set_default_registry(&mut self, id: String, name: String) {
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

    pub(super) fn submit_form(&mut self, form: Form) {
        match form.kind.clone() {
            FormKind::CreateContainer => self.create_container(form),
            FormKind::Node { id } => match id {
                Some(id) => self.edit_node(form, id),
                None => self.add_node(form),
            },
            FormKind::AddCompose => self.add_compose(form),
            FormKind::ComposeTemplate { template } => {
                self.create_compose_from_template(form.value(0), template)
            }
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

fn auto_refresh_allowed(tab: Tab, overlay_open: bool, pull_busy: bool, compose_busy: bool) -> bool {
    matches!(tab, Tab::Containers | Tab::Images | Tab::Compose)
        && !overlay_open
        && !pull_busy
        && !compose_busy
}

pub(super) fn format_pull_layer(progress: &ImagePullProgress) -> String {
    let layer = if progress.layer_id.is_empty() {
        "Docker"
    } else {
        progress.layer_id.as_str()
    };
    let size = match (progress.current, progress.total) {
        (Some(current), Some(total)) => {
            format!("{} / {}", human_size(current), human_size(total))
        }
        (Some(current), None) => format!("{} / 未知大小", human_size(current)),
        (None, Some(total)) => format!("- / {}", human_size(total)),
        (None, None) => "未知大小".to_string(),
    };
    format!("{layer}  {}  {size}", progress.status)
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

#[cfg(test)]
mod tests {
    use super::{auto_refresh_allowed, format_pull_layer, PullOperation};
    use crate::docker::image::ImagePullProgress;
    use crate::tui::Tab;

    fn progress(layer_id: &str, current: Option<i64>, total: Option<i64>) -> ImagePullProgress {
        ImagePullProgress {
            layer_id: layer_id.to_string(),
            status: "Downloading".to_string(),
            current,
            total,
        }
    }

    #[test]
    fn automatic_refresh_only_runs_on_unobstructed_data_pages() {
        for tab in [Tab::Containers, Tab::Images, Tab::Compose] {
            assert!(auto_refresh_allowed(tab, false, false, false));
            assert!(!auto_refresh_allowed(tab, true, false, false));
            assert!(!auto_refresh_allowed(tab, false, true, false));
            assert!(!auto_refresh_allowed(tab, false, false, true));
        }
        assert!(!auto_refresh_allowed(Tab::Nodes, false, false, false));
        assert!(!auto_refresh_allowed(Tab::Registries, false, false, false));
        assert!(!auto_refresh_allowed(Tab::Help, false, false, false));
    }

    #[test]
    fn pull_progress_updates_layers_without_fabricating_percentages() {
        let mut operation = PullOperation {
            id: 7,
            reference: "alpine:latest".to_string(),
            layers: Vec::new(),
        };
        operation.apply(progress("layer-a", Some(512), None));
        operation.apply(progress("layer-b", Some(1024), Some(4096)));
        operation.apply(progress("layer-a", Some(1024), None));

        assert_eq!(operation.layers.len(), 2);
        assert_eq!(operation.layers[0].current, Some(1024));
        let unknown = format_pull_layer(&operation.layers[0]);
        assert!(unknown.contains("layer-a"));
        assert!(unknown.contains("Downloading"));
        assert!(unknown.contains("未知大小"));
        assert!(!unknown.contains('%'));
        let known = format_pull_layer(&operation.layers[1]);
        assert!(known.contains("1.0KB / 4.0KB"));
        assert!(!known.contains('%'));
    }
}
