use bollard::query_parameters::{
    CreateImageOptions, ImportImageOptions, ListImagesOptions, RemoveImageOptions,
    SearchImagesOptions,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

use super::connect;
#[cfg(feature = "desktop")]
use crate::config::load_state;
use crate::config::{load_registries, RegistrySource, ShipNode};
use crate::error::AppError;
#[cfg(feature = "desktop")]
use crate::error::{respond, ApiResponse};

const MAX_OUTPUT_LINES: usize = 80;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub id: String,
    pub short_id: String,
    pub repo_tags: Vec<String>,
    pub repo_digests: Vec<String>,
    pub size: i64,
    pub created: i64,
    pub containers: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSearchResult {
    pub name: String,
    pub description: String,
    pub star_count: i64,
    pub is_official: bool,
}

#[derive(Debug, Deserialize)]
struct DockerHubSearchResponse {
    #[serde(default)]
    results: Vec<DockerHubSearchResult>,
}

#[derive(Debug, Deserialize)]
struct DockerHubSearchResult {
    repo_name: String,
    short_description: Option<String>,
    star_count: Option<i64>,
    is_official: Option<bool>,
}

impl From<DockerHubSearchResult> for ImageSearchResult {
    fn from(result: DockerHubSearchResult) -> Self {
        Self {
            name: result.repo_name,
            description: result.short_description.unwrap_or_default(),
            star_count: result.star_count.unwrap_or(0),
            is_official: result.is_official.unwrap_or(false),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageOperationResult {
    pub reference: String,
    pub output: String,
}

struct RegistryAddress {
    prefix: String,
    host: String,
}

#[cfg(feature = "desktop")]
fn node_by_id(node_id: &str) -> Result<ShipNode, AppError> {
    load_state()?
        .nodes
        .into_iter()
        .find(|node| node.id == node_id)
        .ok_or_else(|| AppError::validation("找不到当前舰船，请返回舰队总览重新选择。"))
}

fn required<'a>(value: &'a str, message: &str) -> Result<&'a str, AppError> {
    let value = value.trim();
    if value.is_empty() {
        Err(AppError::input(message))
    } else {
        Ok(value)
    }
}

fn validate_port(port: &str) -> Result<(), AppError> {
    if port.parse::<u16>().ok().is_some_and(|port| port > 0) {
        Ok(())
    } else {
        Err(AppError::input("镜像源地址包含无效端口。"))
    }
}

fn parse_registry_url(url: &str) -> Result<RegistryAddress, AppError> {
    let url = required(url, "镜像源地址不能为空。")?;
    if url.contains(['?', '#']) {
        return Err(AppError::input("镜像源地址不能包含查询参数或片段。"));
    }
    let (scheme, remainder) = url
        .split_once("://")
        .ok_or_else(|| AppError::input("镜像源地址需以 http:// 或 https:// 开头。"))?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return Err(AppError::input("镜像源地址仅支持 HTTP 或 HTTPS 协议。"));
    }
    let authority = remainder.split('/').next().unwrap_or_default();
    if authority.is_empty() || authority.chars().any(char::is_whitespace) {
        return Err(AppError::input("镜像源地址缺少有效主机。"));
    }
    if authority.contains('@') {
        return Err(AppError::input("镜像源地址不能包含用户名或密码等凭据。"));
    }

    let host = if authority.starts_with('[') {
        let closing = authority
            .find(']')
            .ok_or_else(|| AppError::input("镜像源地址的 IPv6 主机格式不正确。"))?;
        let suffix = &authority[closing + 1..];
        if !suffix.is_empty() {
            let port = suffix
                .strip_prefix(':')
                .ok_or_else(|| AppError::input("镜像源地址包含无效端口。"))?;
            validate_port(port)?;
        }
        &authority[1..closing]
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) => {
                if host.contains(':') {
                    return Err(AppError::input("IPv6 镜像源主机需要使用方括号。"));
                }
                validate_port(port)?;
                host
            }
            None => authority,
        }
    };
    if host.is_empty() {
        return Err(AppError::input("镜像源地址缺少有效主机。"));
    }

    Ok(RegistryAddress {
        prefix: remainder.trim_end_matches('/').to_string(),
        host: host.to_ascii_lowercase(),
    })
}

fn is_official_registry(host: &str) -> bool {
    matches!(
        host,
        "index.docker.io" | "docker.io" | "registry-1.docker.io"
    )
}

fn has_explicit_registry(reference: &str) -> bool {
    reference
        .split_once('/')
        .map(|(first, _)| {
            first.contains('.') || first.contains(':') || first.eq_ignore_ascii_case("localhost")
        })
        .unwrap_or(false)
}

fn resolve_pull_reference(
    reference: &str,
    registry_id: Option<&str>,
    registries: &[RegistrySource],
) -> Result<String, AppError> {
    let registry = match registry_id {
        Some(id) => Some(
            registries
                .iter()
                .find(|registry| registry.id == id.trim())
                .ok_or_else(|| AppError::input("找不到指定的镜像源，请刷新配置后重试。"))?,
        ),
        None => registries.iter().find(|registry| registry.is_default),
    };
    let Some(registry) = registry else {
        return Ok(reference.to_string());
    };
    let address = parse_registry_url(&registry.url)?;
    if is_official_registry(&address.host) {
        return Ok(reference.to_string());
    }
    if has_explicit_registry(reference) {
        return Err(AppError::input(
            "图纸地址已包含显式镜像仓库，不能再套用自定义镜像源；请移除仓库前缀或改用官方源。",
        ));
    }

    let hub_reference = if reference.contains('/') {
        reference.to_string()
    } else {
        format!("library/{reference}")
    };
    Ok(format!("{}/{hub_reference}", address.prefix))
}

fn append_status(lines: &mut VecDeque<String>, status: Option<String>) {
    let Some(status) = status else {
        return;
    };
    let status = status.trim();
    if status.is_empty() {
        return;
    }
    if lines.len() == MAX_OUTPUT_LINES {
        lines.pop_front();
    }
    lines.push_back(status.to_string());
}

fn join_status(lines: VecDeque<String>) -> String {
    lines.into_iter().collect::<Vec<_>>().join("\n")
}

pub(crate) async fn list_for_node(node: &ShipNode) -> Result<Vec<ImageInfo>, AppError> {
    let images = connect(node)
        .await?
        .list_images(Some(ListImagesOptions {
            all: true,
            digests: true,
            ..Default::default()
        }))
        .await
        .map_err(AppError::docker)?;
    Ok(images
        .into_iter()
        .map(|image| {
            let id = image.id;
            let short_id = id
                .strip_prefix("sha256:")
                .unwrap_or(&id)
                .chars()
                .take(12)
                .collect();
            ImageInfo {
                id,
                short_id,
                repo_tags: image.repo_tags,
                repo_digests: image.repo_digests,
                size: image.size,
                created: image.created,
                containers: image.containers,
            }
        })
        .collect())
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn list_images(node_id: String) -> ApiResponse<Vec<ImageInfo>> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            list_for_node(&node).await
        }
        .await,
    )
}

pub(crate) async fn search_for_node(
    node: &ShipNode,
    term: &str,
) -> Result<Vec<ImageSearchResult>, AppError> {
    let term = required(term, "请输入需要检索的图纸名称。")?;
    match connect(node)
        .await?
        .search_images(SearchImagesOptions {
            term: term.to_string(),
            limit: Some(25),
            filters: None,
        })
        .await
    {
        Ok(results) => Ok(results
            .into_iter()
            .map(|result| ImageSearchResult {
                name: result.name.unwrap_or_default(),
                description: result.description.unwrap_or_default(),
                star_count: result.star_count.unwrap_or(0),
                is_official: result.is_official.unwrap_or(false),
            })
            .collect()),
        Err(_) => search_docker_hub(term).await,
    }
}

async fn search_docker_hub(term: &str) -> Result<Vec<ImageSearchResult>, AppError> {
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| AppError::docker(format!("无法初始化 Docker Hub 检索通道：{error}")))?
        .get("https://hub.docker.com/v2/search/repositories/")
        .query(&[("query", term), ("page_size", "25")])
        .send()
        .await
        .map_err(|error| AppError::docker(format!("Docker Hub 直连检索失败：{error}")))?
        .error_for_status()
        .map_err(|error| AppError::docker(format!("Docker Hub 拒绝了检索请求：{error}")))?
        .json::<DockerHubSearchResponse>()
        .await
        .map_err(|error| AppError::docker(format!("Docker Hub 检索结果无法解析：{error}")))?;
    Ok(response.results.into_iter().map(Into::into).collect())
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn search_images(node_id: String, term: String) -> ApiResponse<Vec<ImageSearchResult>> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            search_for_node(&node, &term).await
        }
        .await,
    )
}

pub(crate) async fn pull_for_node(
    node: &ShipNode,
    reference: &str,
    registry_id: Option<&str>,
) -> Result<ImageOperationResult, AppError> {
    let reference = required(reference, "请输入需要拉取的图纸名称与标签。")?;
    let registries = load_registries()?;
    let pull_reference = resolve_pull_reference(reference, registry_id, &registries)?;
    let docker = connect(node).await?;
    let mut stream = docker.create_image(
        Some(CreateImageOptions {
            from_image: Some(pull_reference.clone()),
            ..Default::default()
        }),
        None,
        None,
    );
    let mut statuses = VecDeque::new();
    while let Some(item) = stream.next().await {
        let info = item.map_err(AppError::docker)?;
        if let Some(error) = info.error {
            return Err(AppError::docker(error));
        }
        append_status(&mut statuses, info.status);
    }
    Ok(ImageOperationResult {
        reference: pull_reference,
        output: join_status(statuses),
    })
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn pull_image(
    node_id: String,
    reference: String,
    registry_id: Option<String>,
) -> ApiResponse<ImageOperationResult> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            pull_for_node(&node, &reference, registry_id.as_deref()).await
        }
        .await,
    )
}

pub(crate) async fn remove_for_node(
    node: &ShipNode,
    image_id: &str,
    force: bool,
) -> Result<bool, AppError> {
    let image_id = required(image_id, "缺少图纸标识，请刷新列表后重试。")?;
    connect(node)
        .await?
        .remove_image(
            image_id,
            Some(RemoveImageOptions {
                force,
                noprune: false,
            }),
            None,
        )
        .await
        .map_err(AppError::docker)?;
    Ok(true)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn remove_image(
    node_id: String,
    image_id: String,
    force: Option<bool>,
) -> ApiResponse<bool> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            remove_for_node(&node, &image_id, force.unwrap_or(false)).await
        }
        .await,
    )
}

pub(crate) async fn export_for_node(
    node: &ShipNode,
    reference: &str,
    destination: &str,
) -> Result<bool, AppError> {
    let reference = required(reference, "请选择需要导出的图纸。")?;
    let destination = required(destination, "请选择图纸导出路径。")?;
    let docker = connect(node).await?;
    let mut file = File::create(destination)
        .await
        .map_err(|error| AppError::input(format!("无法创建图纸导出文件：{error}")))?;
    let mut stream = docker.export_image(reference);
    while let Some(item) = stream.next().await {
        let chunk = item.map_err(AppError::docker)?;
        file.write_all(&chunk)
            .await
            .map_err(|error| AppError::input(format!("写入图纸导出文件失败：{error}")))?;
    }
    file.flush()
        .await
        .map_err(|error| AppError::input(format!("完成图纸导出文件写入失败：{error}")))?;
    Ok(true)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn export_image(
    node_id: String,
    reference: String,
    destination: String,
) -> ApiResponse<bool> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            export_for_node(&node, &reference, &destination).await
        }
        .await,
    )
}

pub(crate) async fn import_for_node(
    node: &ShipNode,
    source: &str,
) -> Result<ImageOperationResult, AppError> {
    let source = required(source, "请选择需要导入的图纸文件。")?;
    let docker = connect(node).await?;
    let file = File::open(source)
        .await
        .map_err(|error| AppError::input(format!("无法打开图纸导入文件：{error}")))?;
    let body = bollard::body_try_stream(ReaderStream::new(file));
    let mut stream = docker.import_image(ImportImageOptions::default(), body, None);
    let mut statuses = VecDeque::new();
    while let Some(item) = stream.next().await {
        let info = item.map_err(AppError::docker)?;
        if let Some(error) = info.error {
            return Err(AppError::docker(error));
        }
        append_status(&mut statuses, info.status);
    }
    let reference = Path::new(source)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(source)
        .to_string();
    Ok(ImageOperationResult {
        reference,
        output: join_status(statuses),
    })
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn import_image(node_id: String, source: String) -> ApiResponse<ImageOperationResult> {
    respond(
        async {
            let node = node_by_id(&node_id)?;
            import_for_node(&node, &source).await
        }
        .await,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_hub_search_results_include_optional_fields() {
        let response: DockerHubSearchResponse = serde_json::from_str(
            r#"{
                "results": [
                    {
                        "repo_name": "alpine",
                        "short_description": "A minimal Docker image",
                        "star_count": 42,
                        "is_official": true
                    },
                    {
                        "repo_name": "example/test",
                        "short_description": null,
                        "star_count": null,
                        "is_official": null
                    }
                ]
            }"#,
        )
        .expect("Docker Hub 响应应能解析");

        let results: Vec<ImageSearchResult> =
            response.results.into_iter().map(Into::into).collect();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "alpine");
        assert_eq!(results[0].description, "A minimal Docker image");
        assert_eq!(results[0].star_count, 42);
        assert!(results[0].is_official);
        assert_eq!(results[1].description, "");
        assert_eq!(results[1].star_count, 0);
        assert!(!results[1].is_official);
    }
}
