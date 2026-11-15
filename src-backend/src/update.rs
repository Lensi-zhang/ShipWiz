//! 版本更新检查：读取 GitHub Release 最新版本并与当前编译版本比较
//!
//! 桌面端通过 `check_update` 命令调用，TUI 在启动后后台检查一次，也可按 `U` 手动复查。

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 发布仓库（GitHub Releases 来源）
const REPOSITORY: &str = "Lensi-zhang/ShipWiz";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// 当前编译版本
    pub current: String,
    /// 远端最新版本（去掉前缀 v）
    pub latest: String,
    /// Release 页面地址
    pub url: String,
    /// Release 说明正文
    pub notes: String,
    /// 发布时间
    pub published_at: String,
}

/// 当前可执行文件的版本，编译期取自 Cargo.toml
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
}

/// 请求 GitHub 最新 Release；有更新返回 `Some`，已是最新返回 `None`
pub async fn check_latest() -> Result<Option<UpdateInfo>, AppError> {
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(format!("ShipWiz/{}", current_version()))
        // 发布页靠 302 跳转给出最新 tag，这里不自动跟随
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(AppError::update)?;

    match fetch_release(&client).await {
        Ok(result) => Ok(result),
        Err(api_error) => {
            // GitHub API 被限流或不可达时，退回解析发布页的跳转地址
            match fetch_latest_tag(&client).await {
                Ok(result) => Ok(result),
                Err(_) => Err(api_error),
            }
        }
    }
}

async fn fetch_release(client: &reqwest::Client) -> Result<Option<UpdateInfo>, AppError> {
    let response = client
        .get(format!(
            "https://api.github.com/repos/{REPOSITORY}/releases/latest"
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| AppError::update(format!("无法访问 GitHub：{error}")))?;

    // 仓库还没有发布任何 Release 时同样视为“无更新”
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(AppError::update(format!(
            "GitHub 返回状态码 {}",
            response.status()
        )));
    }

    let release: Release = response
        .json()
        .await
        .map_err(|error| AppError::update(format!("发布信息解析失败：{error}")))?;
    let latest = normalize_version(&release.tag_name);
    if !is_newer(&latest, current_version()) {
        return Ok(None);
    }
    Ok(Some(UpdateInfo {
        current: current_version().to_string(),
        latest,
        url: release.html_url,
        notes: release.body.unwrap_or_default().trim().to_string(),
        published_at: release
            .published_at
            .unwrap_or_default()
            .chars()
            .take(10)
            .collect(),
    }))
}

/// 备用通道：读取 `releases/latest` 的 302 目标，只拿到版本号与页面地址
async fn fetch_latest_tag(client: &reqwest::Client) -> Result<Option<UpdateInfo>, AppError> {
    let response = client
        .get(format!("https://github.com/{REPOSITORY}/releases/latest"))
        .send()
        .await
        .map_err(|error| AppError::update(format!("无法访问 GitHub：{error}")))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let location = response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    if location.is_empty() {
        return Err(AppError::update(format!(
            "GitHub 返回状态码 {}",
            response.status()
        )));
    }
    let latest = normalize_version(location.rsplit('/').next().unwrap_or_default());
    if !is_newer(&latest, current_version()) {
        return Ok(None);
    }
    Ok(Some(UpdateInfo {
        current: current_version().to_string(),
        latest,
        url: location,
        notes: String::new(),
        published_at: String::new(),
    }))
}

/// 比较两个版本号，`latest` 严格大于 `current` 时为真
fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

/// 去掉前缀 v 与预发布后缀，保留三段数字
fn normalize_version(version: &str) -> String {
    let trimmed = version.trim().trim_start_matches(['v', 'V']);
    trimmed
        .split(['-', '+'])
        .next()
        .unwrap_or(trimmed)
        .to_string()
}

fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let normalized = normalize_version(version);
    let mut parts = normalized.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn check_update() -> crate::error::ApiResponse<Option<UpdateInfo>> {
    crate::error::respond(check_latest().await)
}

#[cfg(test)]
mod tests {
    use super::{is_newer, normalize_version, parse_version};

    #[test]
    fn normalizes_release_tags() {
        assert_eq!(normalize_version("v0.2.3"), "0.2.3");
        assert_eq!(normalize_version("V1.10.0-rc.1"), "1.10.0");
        assert_eq!(normalize_version(" 2.0.0 "), "2.0.0");
    }

    #[test]
    fn parses_versions() {
        assert_eq!(parse_version("v0.2.3"), Some((0, 2, 3)));
        assert_eq!(parse_version("1.10"), Some((1, 10, 0)));
        assert_eq!(parse_version("abc"), None);
    }

    #[test]
    fn detects_newer_versions() {
        assert!(is_newer("0.3.0", "0.2.3"));
        assert!(is_newer("v0.2.10", "0.2.9"));
        assert!(!is_newer("0.2.3", "0.2.3"));
        assert!(!is_newer("0.2.2", "0.2.3"));
        assert!(!is_newer("0.2.3", "0.3.0"));
    }
}
