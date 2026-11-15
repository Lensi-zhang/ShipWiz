#![allow(deprecated)]

use crate::config::{self, NodeConnection, ShipNode};
use crate::docker::connect;
use crate::error::AppError;
#[cfg(feature = "desktop")]
use crate::error::{respond, ApiResponse};
use crate::utils::timestamp_ms;
use bollard::container::{ListContainersOptions, LogsOptions};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
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

#[cfg(feature = "desktop")]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeProjectInput {
    pub name: String,
    pub yaml: String,
}

#[cfg(feature = "desktop")]
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

pub(crate) fn load_projects() -> Result<Vec<ComposeProject>, AppError> {
    Ok(config::read_config_key::<Vec<ComposeProject>>("composeProjects")?.unwrap_or_default())
}

fn write_projects(projects: &[ComposeProject]) -> Result<(), AppError> {
    config::write_config_key("composeProjects", projects)
}

pub(crate) fn compose_directory() -> Result<PathBuf, AppError> {
    Ok(config::config_directory()?.join("compose"))
}

pub(crate) fn write_project_file(project: &ComposeProject) -> Result<PathBuf, AppError> {
    let directory = compose_directory()?;
    std::fs::create_dir_all(&directory).map_err(AppError::config)?;
    let path = directory.join(format!("{}.yaml", project.id));
    std::fs::write(&path, &project.yaml).map_err(AppError::config)?;
    Ok(path)
}

pub(crate) fn save_project(
    name: &str,
    yaml: &str,
    id: Option<&str>,
) -> Result<ComposeProject, AppError> {
    let name = normalize_project_name(name)?;
    let services = validate_yaml(yaml)?;
    if services.is_empty() {
        return Err(AppError::input("编队至少需要一个服务。"));
    }
    let mut projects = load_projects()?;
    let now = timestamp_ms();
    let project = if let Some(id) = id {
        let project = projects
            .iter_mut()
            .find(|project| project.id == id)
            .ok_or_else(|| AppError::input("目标编队不存在或已被移除。"))?;
        project.name = name;
        project.yaml = yaml.trim().to_string();
        project.updated_time = now;
        project.clone()
    } else {
        if projects.iter().any(|project| project.name == name) {
            return Err(AppError::input("编队名称已存在，请更换名称。"));
        }
        let project = ComposeProject {
            id: Uuid::new_v4().to_string(),
            name,
            yaml: yaml.trim().to_string(),
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
    write_projects(&projects)?;
    write_project_file(&project)?;
    Ok(project)
}

pub(crate) fn remove_project(id: &str) -> Result<bool, AppError> {
    let mut projects = load_projects()?;
    let original_len = projects.len();
    projects.retain(|project| project.id != id);
    if projects.len() == original_len {
        return Err(AppError::input("目标编队不存在或已被移除。"));
    }
    write_projects(&projects)?;
    let path = compose_directory()?.join(format!("{id}.yaml"));
    if path.exists() {
        std::fs::remove_file(path).map_err(AppError::config)?;
    }
    Ok(true)
}

pub(crate) fn find_project(selector: &str) -> Result<ComposeProject, AppError> {
    let selector = selector.trim();
    load_projects()?
        .into_iter()
        .find(|project| project.id == selector || project.name == selector)
        .ok_or_else(|| AppError::input(format!("找不到编队“{selector}”，请先执行 compose list。")))
}

pub(crate) async fn up_for_node(
    node: &ShipNode,
    project: &ComposeProject,
) -> Result<ComposeCommandResult, AppError> {
    run_project(node, project, ComposeAction::Up).await
}

pub(crate) async fn down_for_node(
    node: &ShipNode,
    project: &ComposeProject,
) -> Result<ComposeCommandResult, AppError> {
    run_project(node, project, ComposeAction::Down).await
}

async fn run_project(
    node: &ShipNode,
    project: &ComposeProject,
    action: ComposeAction,
) -> Result<ComposeCommandResult, AppError> {
    validate_yaml(&project.yaml)?;
    let path = write_project_file(project)?;
    let action_args = match action {
        ComposeAction::Up => vec!["up", "-d"],
        ComposeAction::Down => vec!["down"],
    };
    let output = run_compose_command(node, &project.name, &path, &action_args).await?;
    Ok(ComposeCommandResult {
        project: project.name.clone(),
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
    let docker_host = match node.connection {
        NodeConnection::Ssh => Some(
            lease
                .local_docker_host()
                .ok_or_else(|| AppError::ssh("SSH 隧道缺少本机 Docker 转发地址。"))?
                .to_string(),
        ),
        _ => None,
    };
    let cert_path = if matches!(node.connection, NodeConnection::Tls) {
        if !node.tls {
            return Err(AppError::validation(
                "远程舰船必须使用 TLS，禁止未加密的 TCP 连接。",
            ));
        }
        Some(
            node.cert_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .ok_or_else(|| AppError::validation("远程舰船缺少 TLS 证书目录。"))?
                .to_string(),
        )
    } else {
        None
    };
    let mut compose_missing = false;
    for cli in docker_clis() {
        let mut command = build_compose_command(
            &cli,
            node,
            docker_host.as_deref(),
            cert_path.as_deref(),
            project,
            path,
            action_args,
        );
        let output = match timeout(
            Duration::from_secs(COMPOSE_TIMEOUT_SECONDS),
            command.output(),
        )
        .await
        {
            Ok(Ok(output)) => output,
            Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Ok(Err(error)) => return Err(AppError::docker(error)),
            Err(_) => return Err(AppError::docker("docker compose command timed out")),
        };
        let mut text = String::from_utf8_lossy(&output.stdout).to_string();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        if !output.status.success() {
            if is_compose_unavailable(&text) {
                compose_missing = true;
                continue;
            }
            return Err(AppError::docker(if text.trim().is_empty() {
                format!("docker compose exited with status {}", output.status)
            } else {
                text
            }));
        }
        return Ok(if text.trim().is_empty() {
            "编队指令执行完成。".to_string()
        } else {
            text
        });
    }
    Err(AppError::compose(if compose_missing {
        "已找到 Docker CLI，但缺少 compose 子命令，请安装 Compose 插件或 docker-compose。"
    } else {
        "未检测到 Docker Compose 命令，请安装带 Compose 插件的 Docker Desktop 或 Docker Engine（也可通过环境变量 SHIPWIZ_DOCKER_PATH 指定 docker 可执行文件路径）。"
    }))
}

struct DockerCli {
    program: std::ffi::OsString,
    legacy_compose: bool,
}

fn docker_clis() -> Vec<DockerCli> {
    let mut clis = Vec::new();
    if let Some(path) = std::env::var_os("SHIPWIZ_DOCKER_PATH") {
        clis.push(DockerCli {
            program: path,
            legacy_compose: false,
        });
    }
    clis.push(DockerCli {
        program: std::ffi::OsString::from("docker"),
        legacy_compose: false,
    });

    #[cfg(windows)]
    {
        let mut dirs: Vec<PathBuf> = Vec::new();
        for root in ["ProgramFiles", "ProgramW6432", "LOCALAPPDATA"] {
            if let Some(base) = std::env::var_os(root) {
                dirs.push(
                    PathBuf::from(base)
                        .join("Docker")
                        .join("Docker")
                        .join("resources")
                        .join("bin"),
                );
            }
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(
                PathBuf::from(&local)
                    .join("Programs")
                    .join("Rancher Desktop")
                    .join("resources")
                    .join("resources")
                    .join("win32")
                    .join("bin"),
            );
            dirs.push(PathBuf::from(local).join("Programs").join("Podman"));
        }
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            dirs.push(PathBuf::from(program_files).join("RedHat").join("Podman"));
        }
        for dir in dirs {
            push_cli(&mut clis, dir.join("docker.exe"), false);
            push_cli(&mut clis, dir.join("docker-compose.exe"), true);
        }
    }

    #[cfg(not(windows))]
    {
        for dir in [
            "/usr/local/bin",
            "/usr/bin",
            "/snap/bin",
            "/opt/homebrew/bin",
        ] {
            push_cli(&mut clis, PathBuf::from(dir).join("docker"), false);
            push_cli(&mut clis, PathBuf::from(dir).join("docker-compose"), true);
        }
    }
    clis
}

fn push_cli(clis: &mut Vec<DockerCli>, path: PathBuf, legacy_compose: bool) {
    if path.is_file() {
        clis.push(DockerCli {
            program: path.into_os_string(),
            legacy_compose,
        });
    }
}

fn is_compose_unavailable(output: &str) -> bool {
    let text = output.to_lowercase();
    text.contains("is not a docker command") || text.contains("unknown command")
}

fn build_compose_command(
    cli: &DockerCli,
    node: &ShipNode,
    docker_host: Option<&str>,
    cert_path: Option<&str>,
    project: &str,
    path: &PathBuf,
    action_args: &[&str],
) -> Command {
    let mut command = Command::new(&cli.program);
    if cli.legacy_compose {
        command
            .arg("--no-ansi")
            .arg("--project-name")
            .arg(project)
            .arg("--file")
            .arg(path);
    } else {
        command
            .arg("compose")
            .arg("--ansi")
            .arg("never")
            .arg("--project-name")
            .arg(project)
            .arg("--file")
            .arg(path);
    }
    command
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
            command
                .env("DOCKER_HOST", format!("tcp://{}:{}", node.host, node.port))
                .env("DOCKER_TLS_VERIFY", "1")
                .env("DOCKER_CERT_PATH", cert_path.unwrap_or_default())
                .env_remove("DOCKER_CONTEXT");
        }
        NodeConnection::Ssh => {
            command
                .env("DOCKER_HOST", docker_host.unwrap_or_default())
                .env_remove("DOCKER_TLS_VERIFY")
                .env_remove("DOCKER_CERT_PATH")
                .env_remove("DOCKER_CONTEXT");
        }
    }
    command
}

pub(crate) async fn list_for_node(node: &ShipNode) -> Result<Vec<ComposeDeployment>, AppError> {
    let docker = connect(node).await?;
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

pub(crate) async fn logs_for_node(
    node: &ShipNode,
    project: &str,
    tail: u32,
) -> Result<String, AppError> {
    let project = project.trim();
    if project.is_empty() {
        return Err(AppError::input("缺少编队名称，请刷新后重试。"));
    }
    let docker = connect(node).await?;
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
                tail: tail.clamp(1, 2000).to_string(),
                ..Default::default()
            }),
        );
        while let Some(line) = stream.next().await {
            output.push_str(&line.map_err(AppError::docker)?.to_string());
        }
    }
    Ok(output)
}

pub(crate) fn validate_yaml(yaml: &str) -> Result<Vec<String>, AppError> {
    let value: Value = serde_yaml::from_str(yaml)
        .map_err(|error| AppError::input(format!("编队指令格式无效：{error}")))?;
    let root = value
        .as_mapping()
        .ok_or_else(|| AppError::input("编队指令顶层必须是 YAML 对象。"))?;
    let services = root
        .get(Value::String("services".to_string()))
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

#[derive(Clone, Copy)]
enum ComposeAction {
    Up,
    Down,
}

#[cfg(feature = "desktop")]
fn node_by_id(node_id: &str) -> Result<ShipNode, AppError> {
    config::load_state()?
        .nodes
        .into_iter()
        .find(|node| node.id == node_id)
        .ok_or_else(|| AppError::validation("找不到当前舰船，请返回舰队总览重新选择。"))
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn list_compose_projects() -> ApiResponse<Vec<ComposeProject>> {
    respond((|| {
        let mut projects = load_projects()?;
        projects.sort_by_key(|project| std::cmp::Reverse(project.updated_time));
        Ok(projects)
    })())
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn save_compose_project(
    input: ComposeProjectInput,
    id: Option<String>,
) -> ApiResponse<ComposeProject> {
    respond(save_project(&input.name, &input.yaml, id.as_deref()))
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn remove_compose_project(id: String) -> ApiResponse<bool> {
    respond(remove_project(&id))
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn validate_compose(yaml: String) -> ApiResponse<ComposeValidation> {
    respond((|| {
        Ok(ComposeValidation {
            services: validate_yaml(&yaml)?,
        })
    })())
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn compose_up(node_id: String, project_id: String) -> ApiResponse<ComposeCommandResult> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            let project = find_project(&project_id)?;
            up_for_node(&node, &project).await
        }
        .await,
    )
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn compose_down(
    node_id: String,
    project_id: String,
) -> ApiResponse<ComposeCommandResult> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            let project = find_project(&project_id)?;
            down_for_node(&node, &project).await
        }
        .await,
    )
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn compose_list(node_id: String) -> ApiResponse<Vec<ComposeDeployment>> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            list_for_node(&node).await
        }
        .await,
    )
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn compose_logs(
    node_id: String,
    project: String,
    tail: Option<u32>,
) -> ApiResponse<String> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            logs_for_node(&node, &project, tail.unwrap_or(200)).await
        }
        .await,
    )
}
