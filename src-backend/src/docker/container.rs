#![allow(deprecated)]

use bollard::container::{
    Config, CreateContainerOptions, ListContainersOptions, LogsOptions, RemoveContainerOptions,
    RestartContainerOptions, StartContainerOptions, StatsOptions, StopContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecOptions, StartExecResults};
use bollard::image::ListImagesOptions;
use bollard::models::{
    ContainerStatsResponse, ContainerSummary, HostConfig, PortBinding, RestartPolicy,
    RestartPolicyNameEnum,
};
use bollard::Docker;
use futures_util::{future::join_all, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;

use super::{connect, CONNECT_TIMEOUT_SECONDS};
#[cfg(feature = "desktop")]
use crate::config::load_state;
use crate::config::ShipNode;
use crate::error::AppError;
#[cfg(feature = "desktop")]
use crate::error::{respond, ApiResponse};

const STATS_TIMEOUT_SECONDS: u64 = 5;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerPort {
    pub private_port: u16,
    pub public_port: Option<u16>,
    pub protocol: String,
    pub host_ip: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerInfo {
    pub id: String,
    pub short_id: String,
    pub name: String,
    pub image: String,
    pub image_id: String,
    pub state: String,
    pub status: String,
    pub created: i64,
    pub ports: Vec<ContainerPort>,
    pub cpu_percent: f64,
    pub memory_usage: u64,
    pub memory_limit: u64,
    pub stats_available: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortInput {
    pub host_port: u16,
    pub container_port: u16,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    #[serde(default)]
    pub host_ip: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeInput {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateContainerInput {
    pub name: String,
    pub image: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub ports: Vec<PortInput>,
    #[serde(default)]
    pub volumes: Vec<VolumeInput>,
    #[serde(default = "default_restart_policy")]
    pub restart_policy: String,
    #[serde(default)]
    pub network: String,
    #[serde(default)]
    pub auto_remove: bool,
    #[serde(default)]
    pub start_after_create: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecResult {
    pub output: String,
    pub exit_code: Option<i64>,
}

#[cfg(feature = "desktop")]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FleetOverview {
    pub containers: usize,
    pub running: usize,
    pub images: usize,
    pub unavailable: usize,
}

fn default_protocol() -> String {
    "tcp".to_string()
}

fn default_restart_policy() -> String {
    "no".to_string()
}

#[cfg(feature = "desktop")]
fn node_by_id(node_id: &str) -> Result<ShipNode, AppError> {
    load_state()?
        .nodes
        .into_iter()
        .find(|node| node.id == node_id)
        .ok_or_else(|| AppError::validation("找不到当前舰船，请返回舰队总览重新选择。"))
}

fn validate_id(id: &str) -> Result<(), AppError> {
    if id.trim().is_empty() {
        Err(AppError::input("缺少集装箱标识，请刷新列表后重试。"))
    } else {
        Ok(())
    }
}

fn apply_stats(container: &mut ContainerInfo, stats: ContainerStatsResponse) {
    let cpu_stats = stats.cpu_stats.unwrap_or_default();
    let previous_cpu_stats = stats.precpu_stats.unwrap_or_default();
    let memory_stats = stats.memory_stats.unwrap_or_default();
    let cpu_usage = cpu_stats.cpu_usage.unwrap_or_default();
    let previous_cpu_usage = previous_cpu_stats.cpu_usage.unwrap_or_default();
    let cpu_delta = cpu_usage
        .total_usage
        .unwrap_or(0)
        .checked_sub(previous_cpu_usage.total_usage.unwrap_or(0))
        .unwrap_or(0);
    let system_delta = cpu_stats
        .system_cpu_usage
        .unwrap_or(0)
        .checked_sub(previous_cpu_stats.system_cpu_usage.unwrap_or(0))
        .unwrap_or(0);
    let cpus = cpu_stats
        .online_cpus
        .filter(|cpus| *cpus > 0)
        .map(u64::from)
        .or_else(|| {
            cpu_usage
                .percpu_usage
                .as_ref()
                .map(|usage| usage.len() as u64)
        })
        .unwrap_or(1)
        .max(1);

    container.cpu_percent = if system_delta > 0 && cpu_delta > 0 {
        (cpu_delta as f64 / system_delta as f64) * cpus as f64 * 100.0
    } else {
        0.0
    };
    container.memory_usage = memory_stats.usage.unwrap_or(0);
    container.memory_limit = memory_stats.limit.unwrap_or(0);
    container.stats_available = true;
}

async fn read_stats(docker: &Docker, container_id: &str) -> Option<ContainerStatsResponse> {
    let mut stream = docker.stats(
        container_id,
        Some(StatsOptions {
            stream: false,
            one_shot: true,
        }),
    );
    match timeout(Duration::from_secs(STATS_TIMEOUT_SECONDS), stream.next()).await {
        Ok(Some(Ok(stats))) => Some(stats),
        _ => None,
    }
}

fn map_container(summary: ContainerSummary) -> ContainerInfo {
    let id = summary.id.unwrap_or_default();
    let short_id = id.chars().take(12).collect::<String>();
    let name = summary
        .names
        .and_then(|names| names.into_iter().next())
        .map(|name| name.trim_start_matches('/').to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| short_id.clone());
    let mut ports = summary
        .ports
        .unwrap_or_default()
        .into_iter()
        .map(|port| ContainerPort {
            private_port: port.private_port,
            public_port: port.public_port,
            protocol: port
                .typ
                .map(|protocol| protocol.to_string())
                .unwrap_or_else(|| "tcp".to_string()),
            host_ip: port.ip,
        })
        .collect::<Vec<_>>();
    ports.sort_by_key(|port| (port.private_port, port.public_port.unwrap_or(0)));
    ports.dedup_by(|left, right| {
        left.private_port == right.private_port
            && left.public_port == right.public_port
            && left.protocol == right.protocol
    });

    ContainerInfo {
        id,
        short_id,
        name,
        image: summary.image.unwrap_or_else(|| "未知图纸".to_string()),
        image_id: summary.image_id.unwrap_or_default(),
        state: summary
            .state
            .map(|state| state.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        status: summary.status.unwrap_or_default(),
        created: summary.created.unwrap_or(0),
        ports,
        cpu_percent: 0.0,
        memory_usage: 0,
        memory_limit: 0,
        stats_available: false,
    }
}

pub(crate) async fn list_for_node(node: &ShipNode) -> Result<Vec<ContainerInfo>, AppError> {
    let docker = connect(node).await?;
    let summaries = docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await
        .map_err(AppError::docker)?;
    let containers = summaries.into_iter().map(map_container).collect::<Vec<_>>();
    let mut containers = join_all(containers.into_iter().map(|mut container| {
        let docker = &docker;
        async move {
            if container.state == "running" {
                if let Some(stats) = read_stats(docker, &container.id).await {
                    apply_stats(&mut container, stats);
                }
            }
            container
        }
    }))
    .await;
    containers.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(containers)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn list_containers(node_id: String) -> ApiResponse<Vec<ContainerInfo>> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            list_for_node(&node).await
        }
        .await,
    )
}

fn validate_create(input: &CreateContainerInput) -> Result<(), AppError> {
    if input.name.trim().is_empty() {
        return Err(AppError::input("请为集装箱填写名称。"));
    }
    if input.image.trim().is_empty() {
        return Err(AppError::input("请填写货箱图纸名称与标签。"));
    }
    if input.auto_remove && !matches!(input.restart_policy.as_str(), "" | "no") {
        return Err(AppError::input(
            "自动撤收不能与重启策略同时启用，请保留其中一项。",
        ));
    }
    for env in &input.env {
        let valid = env
            .split_once('=')
            .map(|(key, _)| !key.trim().is_empty())
            .unwrap_or(false);
        if !valid {
            return Err(AppError::input(format!(
                "环境变量“{env}”格式不正确，应使用 KEY=VALUE。"
            )));
        }
    }
    for port in &input.ports {
        if port.host_port == 0 || port.container_port == 0 {
            return Err(AppError::input("主机与集装箱端口均应在 1–65535 之间。"));
        }
        if !matches!(port.protocol.as_str(), "tcp" | "udp" | "sctp") {
            return Err(AppError::input("端口协议仅支持 TCP、UDP 或 SCTP。"));
        }
    }
    for volume in &input.volumes {
        if volume.source.trim().is_empty() || volume.target.trim().is_empty() {
            return Err(AppError::input("挂载来源与集装箱内路径均不能为空。"));
        }
    }
    if !matches!(
        input.restart_policy.as_str(),
        "" | "no" | "always" | "unless-stopped" | "on-failure"
    ) {
        return Err(AppError::input("无法识别所选重启策略。"));
    }
    Ok(())
}

fn restart_policy(value: &str) -> RestartPolicy {
    let name = match value {
        "always" => RestartPolicyNameEnum::ALWAYS,
        "unless-stopped" => RestartPolicyNameEnum::UNLESS_STOPPED,
        "on-failure" => RestartPolicyNameEnum::ON_FAILURE,
        "no" => RestartPolicyNameEnum::NO,
        _ => RestartPolicyNameEnum::EMPTY,
    };
    RestartPolicy {
        name: Some(name),
        maximum_retry_count: None,
    }
}

pub(crate) async fn create_for_node(
    node: &ShipNode,
    input: CreateContainerInput,
) -> Result<String, AppError> {
    validate_create(&input)?;
    let docker = connect(node).await?;
    let mut exposed_ports = HashMap::new();
    let mut port_bindings = HashMap::new();
    for port in &input.ports {
        let key = format!("{}/{}", port.container_port, port.protocol);
        exposed_ports.insert(key.clone(), HashMap::new());
        port_bindings.insert(
            key,
            Some(vec![PortBinding {
                host_ip: (!port.host_ip.trim().is_empty()).then(|| port.host_ip.trim().to_string()),
                host_port: Some(port.host_port.to_string()),
            }]),
        );
    }
    let binds = input
        .volumes
        .iter()
        .map(|volume| {
            format!(
                "{}:{}:{}",
                volume.source.trim(),
                volume.target.trim(),
                if volume.read_only { "ro" } else { "rw" }
            )
        })
        .collect::<Vec<_>>();
    let volumes = input
        .volumes
        .iter()
        .map(|volume| (volume.target.trim().to_string(), HashMap::new()))
        .collect::<HashMap<_, _>>();
    let command = input
        .command
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let network = (!input.network.trim().is_empty()).then(|| input.network.trim().to_string());
    let config = Config::<String> {
        image: Some(input.image.trim().to_string()),
        cmd: (!command.is_empty()).then_some(command),
        env: (!input.env.is_empty()).then_some(input.env.clone()),
        exposed_ports: (!exposed_ports.is_empty()).then_some(exposed_ports),
        volumes: (!volumes.is_empty()).then_some(volumes),
        host_config: Some(HostConfig {
            binds: (!binds.is_empty()).then_some(binds),
            port_bindings: (!port_bindings.is_empty()).then_some(port_bindings),
            restart_policy: Some(restart_policy(&input.restart_policy)),
            network_mode: network,
            auto_remove: Some(input.auto_remove),
            ..Default::default()
        }),
        ..Default::default()
    };
    let result = docker
        .create_container(
            Some(CreateContainerOptions {
                name: input.name.trim().to_string(),
                platform: None,
            }),
            config,
        )
        .await
        .map_err(AppError::docker)?;
    if input.start_after_create {
        docker
            .start_container(&result.id, None::<StartContainerOptions<String>>)
            .await
            .map_err(AppError::docker)?;
    }
    Ok(result.id)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn create_container(node_id: String, input: CreateContainerInput) -> ApiResponse<String> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            create_for_node(&node, input).await
        }
        .await,
    )
}

pub(crate) async fn start_for_node(node: &ShipNode, container_id: &str) -> Result<bool, AppError> {
    validate_id(container_id)?;
    connect(node)
        .await?
        .start_container(container_id, None::<StartContainerOptions<String>>)
        .await
        .map_err(AppError::docker)?;
    Ok(true)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn start_container(node_id: String, container_id: String) -> ApiResponse<bool> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            start_for_node(&node, &container_id).await
        }
        .await,
    )
}

pub(crate) async fn stop_for_node(node: &ShipNode, container_id: &str) -> Result<bool, AppError> {
    validate_id(container_id)?;
    connect(node)
        .await?
        .stop_container(container_id, Some(StopContainerOptions { t: 10 }))
        .await
        .map_err(AppError::docker)?;
    Ok(true)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn stop_container(node_id: String, container_id: String) -> ApiResponse<bool> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            stop_for_node(&node, &container_id).await
        }
        .await,
    )
}

pub(crate) async fn restart_for_node(
    node: &ShipNode,
    container_id: &str,
) -> Result<bool, AppError> {
    validate_id(container_id)?;
    connect(node)
        .await?
        .restart_container(container_id, Some(RestartContainerOptions { t: 10 }))
        .await
        .map_err(AppError::docker)?;
    Ok(true)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn restart_container(node_id: String, container_id: String) -> ApiResponse<bool> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            restart_for_node(&node, &container_id).await
        }
        .await,
    )
}

pub(crate) async fn remove_for_node(
    node: &ShipNode,
    container_id: &str,
    force: bool,
) -> Result<bool, AppError> {
    validate_id(container_id)?;
    connect(node)
        .await?
        .remove_container(
            container_id,
            Some(RemoveContainerOptions {
                force,
                ..Default::default()
            }),
        )
        .await
        .map_err(AppError::docker)?;
    Ok(true)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn remove_container(node_id: String, container_id: String) -> ApiResponse<bool> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            remove_for_node(&node, &container_id, true).await
        }
        .await,
    )
}

pub(crate) async fn stream_logs_for_node(
    node: &ShipNode,
    container_id: &str,
    tail: u32,
    follow: bool,
    mut sink: impl FnMut(String),
) -> Result<(), AppError> {
    validate_id(container_id)?;
    let docker = connect(node).await?;
    let mut stream = docker.logs(
        container_id,
        Some(LogsOptions::<String> {
            stdout: true,
            stderr: true,
            timestamps: true,
            follow,
            tail: tail.clamp(1, 2000).to_string(),
            ..Default::default()
        }),
    );
    while let Some(line) = stream.next().await {
        sink(line.map_err(AppError::docker)?.to_string());
    }
    Ok(())
}

pub(crate) async fn logs_for_node(
    node: &ShipNode,
    container_id: &str,
    tail: u32,
) -> Result<String, AppError> {
    let mut output = String::new();
    stream_logs_for_node(node, container_id, tail, false, |line| {
        output.push_str(&line);
    })
    .await?;
    Ok(output)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn get_container_logs(
    node_id: String,
    container_id: String,
    tail: Option<u32>,
) -> ApiResponse<String> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            logs_for_node(&node, &container_id, tail.unwrap_or(200)).await
        }
        .await,
    )
}

pub(crate) async fn exec_for_node(
    node: &ShipNode,
    container_id: &str,
    command: &str,
) -> Result<ExecResult, AppError> {
    validate_id(container_id)?;
    if command.trim().is_empty() {
        return Err(AppError::input("请输入需要执行的巡检命令。"));
    }
    let docker = connect(node).await?;
    let exec = docker
        .create_exec(
            container_id,
            CreateExecOptions {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    command.trim().to_string(),
                ]),
                ..Default::default()
            },
        )
        .await
        .map_err(AppError::docker)?;
    let mut output_text = String::new();
    if let StartExecResults::Attached { mut output, .. } = docker
        .start_exec(&exec.id, Some(StartExecOptions::default()))
        .await
        .map_err(AppError::docker)?
    {
        while let Some(line) = output.next().await {
            output_text.push_str(&line.map_err(AppError::docker)?.to_string());
        }
    }
    let inspection = docker
        .inspect_exec(&exec.id)
        .await
        .map_err(AppError::docker)?;
    Ok(ExecResult {
        output: output_text,
        exit_code: inspection.exit_code,
    })
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn exec_container(
    node_id: String,
    container_id: String,
    command: String,
) -> ApiResponse<ExecResult> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            exec_for_node(&node, &container_id, &command).await
        }
        .await,
    )
}

pub(crate) async fn overview_for_node(node: &ShipNode) -> Result<(usize, usize, usize), AppError> {
    timeout(Duration::from_secs(CONNECT_TIMEOUT_SECONDS), async {
        let docker = connect(node).await?;
        let containers = docker
            .list_containers(Some(ListContainersOptions::<String> {
                all: true,
                ..Default::default()
            }))
            .await
            .map_err(AppError::docker)?;
        let images = docker
            .list_images(None::<ListImagesOptions<String>>)
            .await
            .map_err(AppError::docker)?;
        let running = containers
            .iter()
            .filter(|container| {
                container
                    .state
                    .as_ref()
                    .map(|state| state.to_string())
                    .as_deref()
                    == Some("running")
            })
            .count();
        Ok((containers.len(), running, images.len()))
    })
    .await
    .map_err(|_| AppError::docker("connection timed out"))?
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn get_fleet_overview() -> ApiResponse<FleetOverview> {
    respond(
        async {
            let nodes = load_state()?.nodes;
            let results = join_all(nodes.iter().map(overview_for_node)).await;
            let mut overview = FleetOverview::default();
            for result in results {
                match result {
                    Ok((containers, running, images)) => {
                        overview.containers += containers;
                        overview.running += running;
                        overview.images += images;
                    }
                    Err(_) => overview.unavailable += 1,
                }
            }
            Ok(overview)
        }
        .await,
    )
}
