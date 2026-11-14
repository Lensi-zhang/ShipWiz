#![allow(deprecated)]

use crate::config::{load_state, open_store, NodeConnection, ShipNode};
use crate::docker::connect;
use crate::error::{respond, ApiResponse, AppError};
use crate::utils::timestamp_ms;
use bollard::container::{ListContainersOptions, LogsOptions};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::process::Command;
use tokio::time::timeout;
use uuid::Uuid;

const COMPOSE_TIMEOUT_SECONDS: u64 = 600;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeProject {
    pub id: String,
    pub name: String,
    pub yaml: String,
    pub created_time: u64,
    pub updated_time: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeProjectInput {
    pub name: String,
    pub yaml: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeValidation {
    pub services: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeDeployment {
    pub name: String,
    pub containers: usize,
    pub running: usize,
    pub services: Vec<String>,
    pub state: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeCommandResult {
    pub project: String,
    pub output: String,
}

#[tauri::command]
pub async fn list_compose_projects(app: AppHandle) -> ApiResponse<Vec<ComposeProject>> {
    respond((|| {
        let mut projects = load_projects(&app)?;
        projects.sort_by(|left, right| right.updated_time.cmp(&left.updated_time));
        Ok(projects)
    })())
}

#[tauri::command]
pub async fn save_compose_project(
    app: AppHandle,
    input: ComposeProjectInput,
    id: Option<String>,
) -> ApiResponse<ComposeProject> {
    respond((|| {
        let name = normalize_project_name(&input.name)?;
        let services = validate_yaml(&input.yaml)?;
        if services.is_empty() {
            return Err(AppError::input("编队至少需要一个服务。"));
        }
        let mut projects = load_projects(&app)?;
        let now = timestamp_ms();
        let project = if let Some(id) = id {
            let project = projects
                .iter_mut()
                .find(|project| project.id == id)
                .ok_or_else(|| AppError::input("目标编队不存在或已被移除。"))?;
            project.name = name;
            project.yaml = input.yaml.trim().to_string();
            project.updated_time = now;
            project.clone()
        } else {
            if projects.iter().any(|project| project.name == name) {
                return Err(AppError::input("编队名称已存在，请更换名称。"));
            }
            let project = ComposeProject {
                id: Uuid::new_v4().to_string(),
                name,
                yaml: input.yaml.trim().to_string(),
                created_time: now,
                updated_time: now,
            };
            projects.push(project.clone());
            project
        };
        if projects
            .iter()
            .any(|other| other.id != project.id && other.name == project.name)
        {
            return Err(AppError::input("编队名称已存在，请更换名称。"));
        }
        write_projects(&app, &projects)?;
        write_project_file(&app, &project)?;
        Ok(project)
    })())
}

#[tauri::command]
pub async fn remove_compose_project(app: AppHandle, id: String) -> ApiResponse<bool> {
    respond((|| {
        let mut projects = load_projects(&app)?;
        let original_len = projects.len();
        projects.retain(|project| project.id != id);
        if projects.len() == original_len {
            return Err(AppError::input("目标编队不存在或已被移除。"));
        }
        write_projects(&app, &projects)?;
        let path = compose_directory(&app)?.join(format!("{id}.yaml"));
        if path.exists() {
            std::fs::remove_file(path).map_err(|error| AppError::config(error))?;
        }
        Ok(true)
    })())
}

#[tauri::command]
pub async fn validate_compose(yaml: String) -> ApiResponse<ComposeValidation> {
    respond((|| {
        Ok(ComposeValidation {
            services: validate_yaml(&yaml)?,
        })
    })())
}

#[tauri::command]
pub async fn compose_up(
    app: AppHandle,
    node_id: String,
    project_id: String,
) -> ApiResponse<ComposeCommandResult> {
    respond(run_saved_project(&app, &node_id, &project_id, ComposeAction::Up).await)
}

#[tauri::command]
pub async fn compose_down(
    app: AppHandle,
    node_id: String,
    project_id: String,
) -> ApiResponse<ComposeCommandResult> {
    respond(run_saved_project(&app, &node_id, &project_id, ComposeAction::Down).await)
}

#[tauri::command]
pub async fn compose_list(app: AppHandle, node_id: String) -> ApiResponse<Vec<ComposeDeployment>> {
    respond(
        async {
            let node = node_by_id(&app, &node_id)?;
            let docker = connect(&node).await?;
            let containers = docker
                .list_containers(Some(ListContainersOptions::<String> {
                    all: true,
                    ..Default::default()
                }))
                .await
                .map_err(AppError::docker)?;
            let mut grouped = BTreeMap::<String, (usize, usize, Vec<String>)>::new();
            for container in containers {
                let labels = container.labels.unwrap_or_default();
                let Some(project) = labels.get("com.docker.compose.project") else {
                    continue;
                };
                let entry = grouped.entry(project.clone()).or_default();
                entry.0 += 1;
                if container
                    .state
                    .as_ref()
                    .is_some_and(|state| state.to_string() == "running")
                {
                    entry.1 += 1;
                }
                if let Some(service) = labels.get("com.docker.compose.service") {
                    if !entry.2.contains(service) {
                        entry.2.push(service.clone());
                    }
                }
            }
            Ok(grouped
                .into_iter()
                .map(|(name, (containers, running, mut services))| {
                    services.sort();
                    let state = if running == containers {
                        "running"
                    } else if running > 0 {
                        "partial"
                    } else {
                        "stopped"
                    };
                    ComposeDeployment {
                        name,
                        containers,
                        running,
                        services,
                        state: state.to_string(),
                    }
                })
                .collect())
        }
        .await,
    )
}

#[tauri::command]
pub async fn compose_logs(
    app: AppHandle,
    node_id: String,
    project: String,
    tail: Option<u32>,
) -> ApiResponse<String> {
    respond(
        async {
            let project = project.trim();
            if project.is_empty() {
                return Err(AppError::input("缺少编队名称，请刷新后重试。"));
            }
            let node = node_by_id(&app, &node_id)?;
            let docker = connect(&node).await?;
            let mut filters = HashMap::new();
            filters.insert(
                "label".to_string(),
                vec![format!("com.docker.compose.project={project}")],
            );
            let containers = docker
                .list_containers(Some(ListContainersOptions {
                    all: true,
                    filters,
                    ..Default::default()
                }))
                .await
                .map_err(AppError::docker)?;
            let mut output = String::new();
            for container in containers {
                let id = container.id.unwrap_or_default();
                let name = container
                    .names
                    .and_then(|names| names.into_iter().next())
                    .map(|name| name.trim_start_matches('/').to_string())
                    .unwrap_or_else(|| id.chars().take(12).collect());
                output.push_str(&format!("\n===== {name} =====\n"));
                let mut stream = docker.logs(
                    &id,
                    Some(LogsOptions::<String> {
                        stdout: true,
                        stderr: true,
                        timestamps: true,
                        tail: tail.unwrap_or(200).clamp(1, 2000).to_string(),
                        ..Default::default()
                    }),
                );
                while let Some(line) = stream.next().await {
                    output.push_str(&line.map_err(AppError::docker)?.to_string());
                }
            }
            Ok(output)
        }
        .await,
    )
}

#[derive(Clone, Copy)]
enum ComposeAction {
    Up,
    Down,
}

async fn run_saved_project(
    app: &AppHandle,
    node_id: &str,
    project_id: &str,
    action: ComposeAction,
) -> Result<ComposeCommandResult, AppError> {
    let node = node_by_id(app, node_id)?;
    let project = load_projects(app)?
        .into_iter()
        .find(|project| project.id == project_id)
        .ok_or_else(|| AppError::input("请先保存编队，再执行启停操作。"))?;
    validate_yaml(&project.yaml)?;
    let path = write_project_file(app, &project)?;
    let action_args = match action {
        ComposeAction::Up => vec!["up", "-d"],
        ComposeAction::Down => vec!["down"],
    };
    let output = run_compose_command(&node, &project.name, &path, &action_args).await?;
    Ok(ComposeCommandResult {
        project: project.name,
        output,
    })
}

async fn run_compose_command(
    node: &ShipNode,
    project: &str,
    path: &PathBuf,
    action_args: &[&str],
) -> Result<String, AppError> {
    let lease = connect(node).await?;
    let mut command = Command::new("docker");
    command
        .arg("compose")
        .arg("--ansi")
        .arg("never")
        .arg("--project-name")
        .arg(project)
        .arg("--file")
        .arg(path)
        .args(action_args)
        .env("NO_COLOR", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    match node.connection {
        NodeConnection::Local => {
            command.env_remove("DOCKER_HOST");
            command.env_remove("DOCKER_TLS_VERIFY");
            command.env_remove("DOCKER_CERT_PATH");
            command.env_remove("DOCKER_CONTEXT");
        }
        NodeConnection::Tls => {
            if !node.tls {
                return Err(AppError::validation(
                    "远程舰船必须使用 TLS，禁止未加密的 TCP 连接。",
                ));
            }
            let cert_path = node
                .cert_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .ok_or_else(|| AppError::validation("远程舰船缺少 TLS 证书目录。"))?;
            command
                .env("DOCKER_HOST", format!("tcp://{}:{}", node.host, node.port))
                .env("DOCKER_TLS_VERIFY", "1")
                .env("DOCKER_CERT_PATH", cert_path)
                .env_remove("DOCKER_CONTEXT");
        }
        NodeConnection::Ssh => {
            let docker_host = lease
                .local_docker_host()
                .ok_or_else(|| AppError::ssh("SSH 隧道缺少本机 Docker 转发地址。"))?;
            command
                .env("DOCKER_HOST", docker_host)
                .env_remove("DOCKER_TLS_VERIFY")
                .env_remove("DOCKER_CERT_PATH")
                .env_remove("DOCKER_CONTEXT");
        }
    }
    let output = timeout(
        Duration::from_secs(COMPOSE_TIMEOUT_SECONDS),
        command.output(),
    )
    .await
    .map_err(|_| AppError::docker("docker compose command timed out"))?
    .map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::compose("未检测到 Docker Compose 命令，请安装带 Compose 插件的 Docker Desktop 或 Docker Engine。")
        } else {
            AppError::docker(error)
        }
    })?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    if !output.status.success() {
        return Err(AppError::docker(if text.trim().is_empty() {
            format!("docker compose exited with status {}", output.status)
        } else {
            text
        }));
    }
    Ok(if text.trim().is_empty() {
        "编队指令执行完成。".to_string()
    } else {
        text
    })
}

pub(crate) fn validate_yaml(yaml: &str) -> Result<Vec<String>, AppError> {
    let value: Value = serde_yaml::from_str(yaml)
        .map_err(|error| AppError::input(format!("编队指令格式无效：{error}")))?;
    let root = value
        .as_mapping()
        .ok_or_else(|| AppError::input("编队指令顶层必须是 YAML 对象。"))?;
    let services = root
        .get(&Value::String("services".to_string()))
        .and_then(Value::as_mapping)
        .ok_or_else(|| AppError::input("编队指令必须包含 services 服务表。"))?;
    if services.is_empty() {
        return Err(AppError::input("编队至少需要一个服务。"));
    }
    let mut names = Vec::new();
    for (name, definition) in services {
        let name = name
            .as_str()
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| AppError::input("服务名称必须是非空文本。"))?;
        if !definition.is_mapping() {
            return Err(AppError::input(format!("服务 {name} 的配置必须是对象。")));
        }
        names.push(name.to_string());
    }
    names.sort();
    Ok(names)
}

fn normalize_project_name(name: &str) -> Result<String, AppError> {
    let name = name.trim().to_lowercase();
    let valid = !name.is_empty()
        && name.len() <= 63
        && name
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if !valid {
        return Err(AppError::input(
            "编队名称需以字母或数字开头，仅包含小写字母、数字、短横线和下划线，最长 63 位。",
        ));
    }
    Ok(name)
}

fn node_by_id(app: &AppHandle, node_id: &str) -> Result<ShipNode, AppError> {
    load_state(app)?
        .nodes
        .into_iter()
        .find(|node| node.id == node_id)
        .ok_or_else(|| AppError::validation("找不到当前舰船，请返回舰队总览重新选择。"))
}

fn load_projects(app: &AppHandle) -> Result<Vec<ComposeProject>, AppError> {
    let store = open_store(app)?;
    match store.get("composeProjects") {
        Some(value) => serde_json::from_value(value).map_err(|error| AppError::config(error)),
        None => Ok(Vec::new()),
    }
}

fn write_projects(app: &AppHandle, projects: &[ComposeProject]) -> Result<(), AppError> {
    let store = open_store(app)?;
    let value = serde_json::to_value(projects).map_err(|error| AppError::config(error))?;
    store.set("composeProjects", value);
    store.save().map_err(|error| AppError::config(error))
}

fn compose_directory(app: &AppHandle) -> Result<PathBuf, AppError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| AppError::config(error))?;
    Ok(home.join(".shipwiz").join("compose"))
}

fn write_project_file(app: &AppHandle, project: &ComposeProject) -> Result<PathBuf, AppError> {
    let directory = compose_directory(app)?;
    std::fs::create_dir_all(&directory).map_err(|error| AppError::config(error))?;
    let path = directory.join(format!("{}.yaml", project.id));
    std::fs::write(&path, &project.yaml).map_err(|error| AppError::config(error))?;
    Ok(path)
}
