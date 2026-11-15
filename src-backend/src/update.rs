//! 版本更新检查：读取 GitHub Release 最新版本并与当前编译版本比较
//!
//! 桌面端通过 `check_update` 命令调用，TUI 在启动后后台检查一次，也可按 `U` 手动复查。

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 发布仓库（GitHub Releases 来源）
const REPOSITORY: &str = "Lensi-zhang/ShipWiz";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const UPDATE_CACHE_TTL_MS: u64 = 24 * 60 * 60 * 1000;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCache {
    current: String,
    /// Unix 时间戳（毫秒）
    checked_at: u64,
    update: Option<UpdateInfo>,
    #[serde(default)]
    skipped_version: Option<String>,
}

impl UpdateCache {
    fn empty() -> Self {
        Self {
            current: current_version().to_string(),
            checked_at: 0,
            update: None,
            skipped_version: None,
        }
    }
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

/// 自动检查优先读取缓存并应用跳过版本，手动检查始终联网且返回原始结果。
pub async fn check(manual: bool) -> Result<Option<UpdateInfo>, AppError> {
    let cache_path = update_cache_path().ok();

    if !manual {
        if let Some(cache) = cache_path
            .as_ref()
            .and_then(|path| read_cache(path).ok().flatten())
        {
            if cache_is_valid(&cache, current_version(), crate::utils::timestamp_ms()) {
                return Ok(filter_skipped(
                    cache.update,
                    cache.skipped_version.as_deref(),
                ));
            }
        }
    }

    let update = check_latest().await?;
    let mut skipped_version = None;

    if let Some(path) = cache_path {
        let existing = read_cache(&path).ok().flatten();
        skipped_version = existing
            .as_ref()
            .and_then(|cache| cache.skipped_version.clone());
        let mut cache = existing.unwrap_or_else(UpdateCache::empty);
        cache.current = current_version().to_string();
        cache.checked_at = crate::utils::timestamp_ms();
        cache.update = update.clone();

        // 缓存失败不影响检查结果；显式跳过操作仍会返回读写错误。
        let _ = write_cache(&path, &cache);
    }

    if manual {
        Ok(update)
    } else {
        Ok(filter_skipped(update, skipped_version.as_deref()))
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

fn cache_is_valid(cache: &UpdateCache, current: &str, now: u64) -> bool {
    cache.current == current && now.saturating_sub(cache.checked_at) < UPDATE_CACHE_TTL_MS
}

fn filter_skipped(update: Option<UpdateInfo>, skipped_version: Option<&str>) -> Option<UpdateInfo> {
    update.filter(|info| Some(info.latest.as_str()) != skipped_version)
}

fn update_cache_path() -> Result<PathBuf, AppError> {
    Ok(crate::config::config_directory()?.join("update.json"))
}

fn read_cache(path: &Path) -> Result<Option<UpdateCache>, AppError> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(AppError::update(format!(
                "读取更新缓存 {} 失败：{error}",
                path.display()
            )))
        }
    };

    // 损坏或结构不兼容的缓存等同于没有缓存。
    Ok(serde_json::from_str(&content).ok())
}

fn write_cache(path: &Path, cache: &UpdateCache) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            AppError::update(format!(
                "创建更新缓存目录 {} 失败：{error}",
                parent.display()
            ))
        })?;
    }
    let content = serde_json::to_vec_pretty(cache)
        .map_err(|error| AppError::update(format!("序列化更新缓存失败：{error}")))?;
    let file_name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("update.json"));
    let temporary = path.with_file_name(format!(
        ".{}.{}.tmp",
        file_name.to_string_lossy(),
        uuid::Uuid::new_v4()
    ));

    let result = (|| -> Result<(), AppError> {
        std::fs::write(&temporary, content).map_err(|error| {
            AppError::update(format!(
                "写入更新缓存 {} 失败：{error}",
                temporary.display()
            ))
        })?;

        #[cfg(windows)]
        if let Err(error) = std::fs::remove_file(path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(AppError::update(format!(
                    "替换更新缓存 {} 失败：{error}",
                    path.display()
                )));
            }
        }

        std::fs::rename(&temporary, path).map_err(|error| {
            AppError::update(format!("替换更新缓存 {} 失败：{error}", path.display()))
        })?;
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

pub fn skip_version(version: String) -> Result<(), AppError> {
    let path = update_cache_path()?;
    let mut cache = read_cache(&path)?.unwrap_or_else(UpdateCache::empty);
    cache.skipped_version = Some(version);
    write_cache(&path, &cache)
}

pub fn clear_skipped() -> Result<(), AppError> {
    let path = update_cache_path()?;
    let Some(mut cache) = read_cache(&path)? else {
        return Ok(());
    };
    cache.skipped_version = None;
    write_cache(&path, &cache)
}

pub fn skipped_version() -> Result<Option<String>, AppError> {
    Ok(read_cache(&update_cache_path()?)?.and_then(|cache| cache.skipped_version))
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn check_update(force: Option<bool>) -> crate::error::ApiResponse<Option<UpdateInfo>> {
    crate::error::respond(check(force.unwrap_or(false)).await)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn skip_update(version: String) -> crate::error::ApiResponse<()> {
    crate::error::respond(skip_version(version))
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn clear_skipped_update() -> crate::error::ApiResponse<()> {
    crate::error::respond(clear_skipped())
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn get_skipped_update() -> crate::error::ApiResponse<Option<String>> {
    crate::error::respond(skipped_version())
}

#[cfg(test)]
mod tests {
    use super::{
        cache_is_valid, current_version, filter_skipped, is_newer, normalize_version,
        parse_version, read_cache, write_cache, UpdateCache, UpdateInfo, UPDATE_CACHE_TTL_MS,
    };
    use std::path::PathBuf;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(name: &str) -> Self {
            Self(std::env::temp_dir().join(format!(
                "shipwiz-update-{name}-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            )))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn update_info(latest: &str) -> UpdateInfo {
        UpdateInfo {
            current: current_version().to_string(),
            latest: latest.to_string(),
            url: "https://example.com/release".to_string(),
            notes: String::new(),
            published_at: String::new(),
        }
    }

    fn cache(checked_at: u64) -> UpdateCache {
        UpdateCache {
            current: current_version().to_string(),
            checked_at,
            update: Some(update_info("0.3.0")),
            skipped_version: None,
        }
    }

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

    #[test]
    fn cache_expires_at_24_hour_boundary() {
        let cache = cache(1_000);
        assert!(cache_is_valid(&cache, current_version(), 1_000));
        assert!(cache_is_valid(
            &cache,
            current_version(),
            1_000 + UPDATE_CACHE_TTL_MS - 1
        ));
        assert!(!cache_is_valid(
            &cache,
            current_version(),
            1_000 + UPDATE_CACHE_TTL_MS
        ));
    }

    #[test]
    fn cache_is_invalid_for_another_current_version() {
        assert!(!cache_is_valid(&cache(1_000), "0.0.1", 1_000));
    }

    #[test]
    fn skip_only_filters_the_matching_update() {
        let update = Some(update_info("0.3.0"));
        assert!(filter_skipped(update.clone(), Some("0.3.0")).is_none());
        assert_eq!(
            filter_skipped(update, Some("0.4.0"))
                .expect("其他版本不应被过滤")
                .latest,
            "0.3.0"
        );
        assert!(filter_skipped(None, Some("0.3.0")).is_none());
    }

    #[test]
    fn atomic_cache_write_replaces_existing_file() {
        let directory = TestDirectory::new("atomic-write");
        let path = directory.0.join("update.json");
        let mut first = cache(1_000);
        first.skipped_version = Some("0.3.0".to_string());
        write_cache(&path, &first).expect("首次写入缓存");

        let mut second = cache(2_000);
        second.update = None;
        second.skipped_version = Some("0.4.0".to_string());
        write_cache(&path, &second).expect("覆盖已有缓存");

        let stored = read_cache(&path)
            .expect("读取覆盖后的缓存")
            .expect("缓存文件存在");
        assert_eq!(stored.checked_at, 2_000);
        assert!(stored.update.is_none());
        assert_eq!(stored.skipped_version.as_deref(), Some("0.4.0"));
        assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[test]
    fn damaged_cache_json_is_ignored() {
        let directory = TestDirectory::new("damaged-json");
        std::fs::create_dir_all(&directory.0).expect("创建临时目录");
        let path = directory.0.join("update.json");
        std::fs::write(&path, "{ damaged json").expect("写入损坏缓存");

        assert!(read_cache(&path).expect("损坏缓存应被忽略").is_none());
    }
}
