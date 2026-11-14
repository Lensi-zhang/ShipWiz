use crate::config;
use crate::error::{respond, ApiResponse, AppError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeTemplate {
    pub id: String,
    pub title: String,
    pub desc: String,
    pub tag: Vec<String>,
    pub yaml: String,
    #[serde(default)]
    pub built_in: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeTemplateInput {
    pub title: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub tag: Vec<String>,
    pub yaml: String,
}

#[tauri::command]
pub async fn list_compose_templates() -> ApiResponse<Vec<ComposeTemplate>> {
    respond((|| {
        let mut templates = built_in_templates();
        templates.extend(load_custom_templates()?);
        Ok(templates)
    })())
}

#[tauri::command]
pub async fn save_compose_template(
    input: ComposeTemplateInput,
    id: Option<String>,
) -> ApiResponse<Vec<ComposeTemplate>> {
    respond((|| {
        let title = input.title.trim().to_string();
        let yaml = input.yaml.trim().to_string();
        if title.is_empty() || yaml.is_empty() {
            return Err(AppError::input("模板名称与编队指令不能为空。"));
        }
        crate::compose::validate_yaml(&yaml)?;
        if id.as_deref().is_some_and(|id| id.starts_with("builtin-")) {
            return Err(AppError::input("内置模板不可修改，请另存为本地模板。"));
        }
        let mut templates = load_custom_templates()?;
        let template = ComposeTemplate {
            id: id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            title,
            desc: input.desc.trim().to_string(),
            tag: input
                .tag
                .into_iter()
                .map(|tag| tag.trim().to_string())
                .filter(|tag| !tag.is_empty())
                .collect(),
            yaml,
            built_in: false,
        };
        if let Some(existing) = templates
            .iter_mut()
            .find(|existing| existing.id == template.id)
        {
            *existing = template;
        } else {
            templates.push(template);
        }
        write_custom_templates(&templates)?;
        let mut all = built_in_templates();
        all.extend(templates);
        Ok(all)
    })())
}

#[tauri::command]
pub async fn remove_compose_template(id: String) -> ApiResponse<Vec<ComposeTemplate>> {
    respond((|| {
        if id.starts_with("builtin-") {
            return Err(AppError::input("内置模板不可移除。"));
        }
        let mut templates = load_custom_templates()?;
        let original_len = templates.len();
        templates.retain(|template| template.id != id);
        if templates.len() == original_len {
            return Err(AppError::input("目标本地模板不存在或已被移除。"));
        }
        write_custom_templates(&templates)?;
        let mut all = built_in_templates();
        all.extend(templates);
        Ok(all)
    })())
}

fn load_custom_templates() -> Result<Vec<ComposeTemplate>, AppError> {
    Ok(config::read_config_key::<Vec<ComposeTemplate>>("composeTemplates")?.unwrap_or_default())
}

fn write_custom_templates(templates: &[ComposeTemplate]) -> Result<(), AppError> {
    config::write_config_key("composeTemplates", templates)
}

fn built_in_templates() -> Vec<ComposeTemplate> {
    vec![
        ComposeTemplate {
            id: "builtin-web".to_string(),
            title: "静海 Web 服务".to_string(),
            desc: "适合静态站点与反向代理的轻量 Web 服务。".to_string(),
            tag: vec!["Web".to_string(), "新手".to_string()],
            yaml: r#"services:
  web:
    image: nginx:1.27-alpine
    ports:
      - "8080:80"
    restart: unless-stopped
"#
            .to_string(),
            built_in: true,
        },
        ComposeTemplate {
            id: "builtin-redis".to_string(),
            title: "灯塔缓存".to_string(),
            desc: "带持久化储物舱的单服务缓存编队。".to_string(),
            tag: vec!["缓存".to_string(), "数据".to_string()],
            yaml: r#"services:
  cache:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - cache-data:/data
    restart: unless-stopped
volumes:
  cache-data:
"#
            .to_string(),
            built_in: true,
        },
        ComposeTemplate {
            id: "builtin-postgres".to_string(),
            title: "深水数据库".to_string(),
            desc: "仅供本地开发的数据库模板，请先修改访问口令。".to_string(),
            tag: vec!["数据库".to_string(), "开发".to_string()],
            yaml: r#"services:
  database:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: shipwiz
      POSTGRES_USER: shipwiz
      POSTGRES_PASSWORD: change-me-before-use
    volumes:
      - database-data:/var/lib/postgresql/data
    restart: unless-stopped
volumes:
  database-data:
"#
            .to_string(),
            built_in: true,
        },
    ]
}
