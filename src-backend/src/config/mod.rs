#[cfg(feature = "desktop")]
pub mod commands;

use crate::credential::{self, StoredCredential};
use crate::error::AppError;
use crate::utils::timestamp_ms;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Local,
    Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NodeConnection {
    Local,
    #[default]
    Tls,
    Ssh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SshAuthMethod {
    Password,
    PrivateKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    Online,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipNode {
    pub id: String,
    pub name: String,
    pub alias: String,
    #[serde(rename = "type")]
    pub node_type: NodeType,
    #[serde(default)]
    pub connection: NodeConnection,
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub cert_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_method: Option<SshAuthMethod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_key_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_socket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_success_time: Option<u64>,
    pub status: NodeStatus,
    pub create_time: u64,
}

impl ShipNode {
    fn local() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: "本机旗舰".to_string(),
            alias: "自动探测本机动力引擎".to_string(),
            node_type: NodeType::Local,
            connection: NodeConnection::Local,
            host: "localhost".to_string(),
            port: 0,
            tls: false,
            cert_path: None,
            username: None,
            auth_method: None,
            private_key_path: None,
            remote_socket: None,
            credential_id: None,
            last_success_time: None,
            status: NodeStatus::Offline,
            create_time: timestamp_ms(),
        }
    }

    pub fn normalize(&mut self) {
        if self.node_type == NodeType::Local {
            self.connection = NodeConnection::Local;
            self.host = "localhost".to_string();
            self.port = 0;
            self.tls = false;
            self.cert_path = None;
        } else if self.connection == NodeConnection::Local {
            self.connection = NodeConnection::Tls;
        }

        if self.connection == NodeConnection::Ssh {
            self.tls = false;
            self.cert_path = None;
            if self.port == 0 {
                self.port = 22;
            }
            if self
                .remote_socket
                .as_deref()
                .map_or(true, |value| value.trim().is_empty())
            {
                self.remote_socket = Some("/var/run/docker.sock".to_string());
            }
        } else {
            self.username = None;
            self.auth_method = None;
            self.private_key_path = None;
            self.remote_socket = None;
            self.credential_id = None;
        }
    }

    pub fn same_connection(&self, other: &Self) -> bool {
        self.id == other.id
            && self.node_type == other.node_type
            && self.connection == other.connection
            && self.host == other.host
            && self.port == other.port
            && self.tls == other.tls
            && self.cert_path == other.cert_path
            && self.username == other.username
            && self.auth_method == other.auth_method
            && self.private_key_path == other.private_key_path
            && self.remote_socket == other.remote_socket
            && self.credential_id == other.credential_id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInput {
    pub name: String,
    pub alias: String,
    #[serde(rename = "type")]
    pub node_type: NodeType,
    #[serde(default)]
    pub connection: NodeConnection,
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub cert_path: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub auth_method: Option<SshAuthMethod>,
    #[serde(default)]
    pub private_key_path: Option<String>,
    #[serde(default)]
    pub remote_socket: Option<String>,
    #[serde(default, skip_serializing)]
    pub password: Option<String>,
    #[serde(default, skip_serializing)]
    pub private_key: Option<String>,
    #[serde(default, skip_serializing)]
    pub passphrase: Option<String>,
    #[serde(default, skip_serializing)]
    pub clear_credential: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FleetState {
    pub nodes: Vec<ShipNode>,
    pub current_node_id: Option<String>,
}

impl FleetState {
    fn initial() -> Self {
        let node = ShipNode::local();
        Self {
            current_node_id: Some(node.id.clone()),
            nodes: vec![node],
        }
    }
}

pub(crate) struct PreparedNode {
    pub node: ShipNode,
    pub save_credential: Option<(String, StoredCredential)>,
    pub delete_credentials: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrySource {
    pub id: String,
    pub name: String,
    pub url: String,
    pub is_default: bool,
}

impl RegistrySource {
    fn official() -> Self {
        Self {
            id: "docker-hub".to_string(),
            name: "Docker Hub 官方港".to_string(),
            url: "https://index.docker.io".to_string(),
            is_default: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryInput {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub is_default: bool,
}

pub fn home_directory() -> Result<PathBuf, AppError> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| {
            AppError::config("无法定位用户主目录，请设置 HOME 或 USERPROFILE 环境变量。")
        })
}

pub fn config_directory() -> Result<PathBuf, AppError> {
    Ok(home_directory()?.join(".shipwiz"))
}

pub fn default_config_path() -> Result<PathBuf, AppError> {
    Ok(config_directory()?.join("config.json"))
}

pub(crate) fn read_config_map() -> Result<serde_json::Map<String, serde_json::Value>, AppError> {
    let path = default_config_path()?;
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(serde_json::Map::new())
        }
        Err(error) => {
            return Err(AppError::config(format!(
                "读取配置文件 {} 失败：{error}",
                path.display()
            )))
        }
    };
    if content.trim().is_empty() {
        return Ok(serde_json::Map::new());
    }
    match serde_json::from_str::<serde_json::Value>(&content) {
        Ok(serde_json::Value::Object(map)) => Ok(map),
        Ok(_) => Err(AppError::config(format!(
            "配置文件 {} 的顶层必须是 JSON 对象。",
            path.display()
        ))),
        Err(error) => Err(AppError::config(format!(
            "配置文件 {} 不是有效 JSON：{error}",
            path.display()
        ))),
    }
}

pub(crate) fn write_config_map(
    map: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), AppError> {
    let path = default_config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            AppError::config(format!("无法创建配置目录 {}：{error}", parent.display()))
        })?;
    }
    let content = serde_json::to_string_pretty(map)
        .map_err(|error| AppError::config(format!("整理配置文件失败：{error}")))?;
    std::fs::write(&path, content)
        .map_err(|error| AppError::config(format!("写入配置文件 {} 失败：{error}", path.display())))
}

pub(crate) fn read_config_key<T: DeserializeOwned>(key: &str) -> Result<Option<T>, AppError> {
    match read_config_map()?.remove(key) {
        Some(value) => serde_json::from_value(value)
            .map(Some)
            .map_err(|error| AppError::config(format!("配置项 {key} 格式无效：{error}"))),
        None => Ok(None),
    }
}

pub(crate) fn write_config_key<T: Serialize + ?Sized>(
    key: &str,
    value: &T,
) -> Result<(), AppError> {
    let mut map = read_config_map()?;
    let value = serde_json::to_value(value)
        .map_err(|error| AppError::config(format!("整理配置项 {key} 失败：{error}")))?;
    map.insert(key.to_string(), value);
    write_config_map(&map)
}

fn normalize_state(mut state: FleetState) -> FleetState {
    for node in &mut state.nodes {
        node.normalize();
    }
    let current_is_valid = state
        .current_node_id
        .as_ref()
        .is_some_and(|id| state.nodes.iter().any(|node| &node.id == id));
    if !current_is_valid {
        state.current_node_id = state.nodes.first().map(|node| node.id.clone());
    }
    state
}

pub fn load_state() -> Result<FleetState, AppError> {
    match read_config_key::<FleetState>("fleet")? {
        Some(state) => Ok(normalize_state(state)),
        None => {
            let state = FleetState::initial();
            write_state(&state)?;
            Ok(state)
        }
    }
}

pub fn write_state(state: &FleetState) -> Result<(), AppError> {
    write_config_key("fleet", state)
}

pub fn load_registries() -> Result<Vec<RegistrySource>, AppError> {
    match read_config_key::<Vec<RegistrySource>>("registries")? {
        Some(registries) => Ok(registries),
        None => {
            let registries = vec![RegistrySource::official()];
            write_registries(&registries)?;
            Ok(registries)
        }
    }
}

pub fn write_registries(registries: &[RegistrySource]) -> Result<(), AppError> {
    write_config_key("registries", registries)
}

pub(crate) fn find_node_mut<'a>(
    state: &'a mut FleetState,
    id: &str,
) -> Result<&'a mut ShipNode, AppError> {
    state
        .nodes
        .iter_mut()
        .find(|node| node.id == id)
        .ok_or_else(|| AppError::validation("目标舰船不存在或已被移出舰队。"))
}

pub(crate) fn prepare_node(
    input: NodeInput,
    existing: Option<&ShipNode>,
) -> Result<PreparedNode, AppError> {
    let input = normalize_input(input)?;
    let previous_credential_id = existing.and_then(|node| node.credential_id.clone());
    let can_reuse = existing.is_some_and(|node| {
        node.connection == NodeConnection::Ssh && node.auth_method == input.auth_method
    });
    let mut credential =
        if input.connection == NodeConnection::Ssh && can_reuse && !input.clear_credential {
            if let Some(id) = previous_credential_id.as_deref() {
                credential::load(id).unwrap_or_default()
            } else {
                StoredCredential::default()
            }
        } else {
            StoredCredential::default()
        };

    if input.connection == NodeConnection::Ssh {
        merge_credential(&input, &mut credential);
        validate_credential(&input, &credential)?;
    }

    let credential_id = if input.connection == NodeConnection::Ssh && !credential.is_empty() {
        Some(
            previous_credential_id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
        )
    } else {
        None
    };
    let save_credential = credential_id.clone().map(|id| (id, credential));
    let delete_credentials = previous_credential_id
        .filter(|old_id| credential_id.as_ref() != Some(old_id))
        .into_iter()
        .collect();
    let mut node = ShipNode {
        id: existing.map_or_else(|| Uuid::new_v4().to_string(), |node| node.id.clone()),
        name: input.name,
        alias: input.alias,
        node_type: input.node_type,
        connection: input.connection,
        host: input.host,
        port: input.port,
        tls: input.tls,
        cert_path: input.cert_path,
        username: input.username,
        auth_method: input.auth_method,
        private_key_path: input.private_key_path,
        remote_socket: input.remote_socket,
        credential_id,
        last_success_time: None,
        status: NodeStatus::Offline,
        create_time: existing.map_or_else(timestamp_ms, |node| node.create_time),
    };
    node.normalize();
    Ok(PreparedNode {
        node,
        save_credential,
        delete_credentials,
    })
}

fn merge_credential(input: &NodeInput, credential: &mut StoredCredential) {
    if let Some(password) = supplied_secret(&input.password) {
        credential.password = Some(password);
    }
    if let Some(private_key) = supplied_secret(&input.private_key) {
        credential.private_key = Some(private_key);
    }
    if let Some(passphrase) = supplied_secret(&input.passphrase) {
        credential.passphrase = Some(passphrase);
    }
    match input.auth_method {
        Some(SshAuthMethod::Password) => {
            credential.private_key = None;
            credential.passphrase = None;
        }
        Some(SshAuthMethod::PrivateKey) => {
            credential.password = None;
        }
        None => {}
    }
}

fn supplied_secret(value: &Option<String>) -> Option<String> {
    value.as_ref().filter(|value| !value.is_empty()).cloned()
}

fn validate_credential(input: &NodeInput, credential: &StoredCredential) -> Result<(), AppError> {
    let password_from_env = std::env::var_os("SHIPWIZ_SSH_PASSWORD").is_some();
    let key_from_env = std::env::var_os("SHIPWIZ_SSH_PRIVATE_KEY").is_some();
    match input.auth_method {
        Some(SshAuthMethod::Password)
            if credential.password.as_deref().map_or(true, str::is_empty) && !password_from_env =>
        {
            Err(AppError::validation(
                "SSH 密码不能为空：请通过 --password 传入，或设置 SHIPWIZ_SSH_PASSWORD 环境变量。",
            ))
        }
        Some(SshAuthMethod::PrivateKey)
            if credential
                .private_key
                .as_deref()
                .map_or(true, str::is_empty)
                && input.private_key_path.is_none()
                && !key_from_env =>
        {
            Err(AppError::validation(
                "SSH 私钥认证必须填写私钥文件路径或粘贴私钥内容。",
            ))
        }
        _ => Ok(()),
    }
}

pub(crate) fn normalize_registry(name: String, url: String) -> Result<(String, String), AppError> {
    let name = name.trim().to_string();
    let url = url.trim().trim_end_matches('/').to_string();
    if name.is_empty() || url.is_empty() {
        return Err(AppError::validation("补给港名称与地址不能为空。"));
    }
    let parsed = url::Url::parse(&url)
        .map_err(|_| AppError::validation("补给港地址格式无效，请填写完整的 HTTP(S) 地址。"))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(AppError::validation(
            "补给港仅支持不含凭据、查询参数的 HTTP(S) 地址。",
        ));
    }
    Ok((name, url))
}

pub(crate) fn ensure_default_registry(
    registries: &mut [RegistrySource],
    default_id: Option<&str>,
) -> Result<(), AppError> {
    if registries.is_empty() {
        return Err(AppError::validation("至少需要保留一个补给港。"));
    }
    if let Some(default_id) = default_id {
        for registry in registries.iter_mut() {
            registry.is_default = registry.id == default_id;
        }
    }
    if !registries.iter().any(|registry| registry.is_default) {
        registries[0].is_default = true;
    }
    Ok(())
}

fn normalize_input(mut input: NodeInput) -> Result<NodeInput, AppError> {
    input.name = input.name.trim().to_string();
    input.alias = input.alias.trim().to_string();
    input.host = input.host.trim().to_string();
    input.username = input
        .username
        .and_then(|value| nonempty(value.trim().to_string()));
    input.private_key_path = input
        .private_key_path
        .and_then(|value| nonempty(value.trim().to_string()));
    input.remote_socket = input
        .remote_socket
        .and_then(|value| nonempty(value.trim().to_string()));
    if input.name.is_empty() {
        return Err(AppError::validation("舰船名称不能为空。"));
    }

    if input.node_type == NodeType::Local {
        input.connection = NodeConnection::Local;
    } else if input.connection == NodeConnection::Local {
        input.connection = NodeConnection::Tls;
    }

    match input.connection {
        NodeConnection::Local => {
            input.host = "localhost".to_string();
            input.port = 0;
            input.tls = false;
            input.cert_path = None;
        }
        NodeConnection::Tls => {
            validate_remote_host(&input.host)?;
            if input.port == 0 {
                return Err(AppError::validation("远程舰船必须填写有效服务端口。"));
            }
            if !input.tls {
                return Err(AppError::validation(
                    "远程舰船必须使用 TLS，禁止未加密的 TCP 连接。",
                ));
            }
            input.cert_path = input
                .cert_path
                .and_then(|path| nonempty(path.trim().to_string()));
            if input.cert_path.is_none() {
                return Err(AppError::validation("远程舰船必须填写 TLS 证书目录。"));
            }
        }
        NodeConnection::Ssh => {
            validate_remote_host(&input.host)?;
            if input.port == 0 {
                input.port = 22;
            }
            input.tls = false;
            input.cert_path = None;
            if input.username.is_none() {
                return Err(AppError::validation("SSH 节点必须填写用户名。"));
            }
            if input.auth_method.is_none() {
                input.auth_method = Some(SshAuthMethod::Password);
            }
            if input.remote_socket.is_none() {
                input.remote_socket = Some("/var/run/docker.sock".to_string());
            }
            if !input
                .remote_socket
                .as_deref()
                .is_some_and(|path| path.starts_with('/'))
            {
                return Err(AppError::validation(
                    "远端 Docker Socket 必须是 Unix Socket 绝对路径。",
                ));
            }
        }
    }
    Ok(input)
}

fn validate_remote_host(host: &str) -> Result<(), AppError> {
    if host.is_empty() {
        return Err(AppError::validation("远程舰船必须填写主机地址。"));
    }
    if host.contains("://") {
        return Err(AppError::validation(
            "主机地址请仅填写 IP 或域名，不要包含协议前缀。",
        ));
    }
    Ok(())
}

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_remote_tls_node_loads_without_losing_fields() {
        let legacy = serde_json::json!({
            "id": "node-1",
            "name": "旧节点",
            "alias": "legacy",
            "type": "remote",
            "host": "10.0.0.8",
            "port": 2376,
            "tls": true,
            "certPath": "C:/certs/node-1",
            "status": "offline",
            "createTime": 123
        });
        let mut node: ShipNode = serde_json::from_value(legacy).unwrap();
        node.normalize();
        assert_eq!(node.connection, NodeConnection::Tls);
        assert_eq!(node.host, "10.0.0.8");
        assert_eq!(node.port, 2376);
        assert!(node.tls);
        assert_eq!(node.cert_path.as_deref(), Some("C:/certs/node-1"));
    }

    #[test]
    fn legacy_local_node_is_normalized_to_local_connection() {
        let legacy = serde_json::json!({
            "id": "local",
            "name": "本机",
            "alias": "",
            "type": "local",
            "host": "localhost",
            "port": 0,
            "tls": false,
            "certPath": null,
            "status": "offline",
            "createTime": 123
        });
        let mut node: ShipNode = serde_json::from_value(legacy).unwrap();
        node.normalize();
        assert_eq!(node.connection, NodeConnection::Local);
        assert_eq!(node.host, "localhost");
    }

    #[test]
    fn blank_secrets_reuse_existing_values() {
        let input: NodeInput = serde_json::from_value(serde_json::json!({
            "name": "SSH",
            "alias": "",
            "type": "remote",
            "connection": "ssh",
            "host": "localhost",
            "port": 22,
            "tls": false,
            "certPath": null,
            "username": "root",
            "authMethod": "privateKey",
            "privateKey": "",
            "passphrase": ""
        }))
        .unwrap();
        let mut credential = StoredCredential {
            private_key: Some("existing-key".to_string()),
            passphrase: Some("existing-passphrase".to_string()),
            ..StoredCredential::default()
        };
        merge_credential(&input, &mut credential);
        assert_eq!(credential.private_key.as_deref(), Some("existing-key"));
        assert_eq!(
            credential.passphrase.as_deref(),
            Some("existing-passphrase")
        );
    }

    #[test]
    fn ssh_defaults_are_applied() {
        let input: NodeInput = serde_json::from_value(serde_json::json!({
            "name": "SSH",
            "alias": "",
            "type": "remote",
            "connection": "ssh",
            "host": "localhost",
            "port": 0,
            "tls": true,
            "certPath": "should-be-cleared",
            "username": "root",
            "authMethod": "password",
            "password": "secret"
        }))
        .unwrap();
        let input = normalize_input(input).unwrap();
        assert_eq!(input.port, 22);
        assert_eq!(input.remote_socket.as_deref(), Some("/var/run/docker.sock"));
        assert!(!input.tls);
        assert!(input.cert_path.is_none());
    }
}
