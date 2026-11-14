pub mod commands;

use crate::error::AppError;
use crate::utils::timestamp_ms;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_store::{Store, StoreExt};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Local,
    Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeConnection {
    Local,
    Tls,
    Ssh,
}

impl Default for NodeConnection {
    fn default() -> Self {
        Self::Tls
    }
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

fn config_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| AppError::config(error))?;
    Ok(home.join(".shipwiz").join("config.json"))
}

pub(crate) fn open_store(app: &AppHandle) -> Result<Arc<Store<Wry>>, AppError> {
    app.store(config_path(app)?)
        .map_err(|error| AppError::config(error))
}

pub fn load_state(app: &AppHandle) -> Result<FleetState, AppError> {
    let store = open_store(app)?;
    match store.get("fleet") {
        Some(value) => {
            let mut state: FleetState =
                serde_json::from_value(value).map_err(|error| AppError::config(error))?;
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
            Ok(state)
        }
        None => {
            let state = FleetState::initial();
            write_state(app, &state)?;
            Ok(state)
        }
    }
}

pub fn write_state(app: &AppHandle, state: &FleetState) -> Result<(), AppError> {
    let store = open_store(app)?;
    let value = serde_json::to_value(state).map_err(|error| AppError::config(error))?;
    store.set("fleet", value);
    store.save().map_err(|error| AppError::config(error))
}

pub fn load_registries(app: &AppHandle) -> Result<Vec<RegistrySource>, AppError> {
    let store = open_store(app)?;
    match store.get("registries") {
        Some(value) => serde_json::from_value(value).map_err(|error| AppError::config(error)),
        None => {
            let registries = vec![RegistrySource::official()];
            write_registries(app, &registries)?;
            Ok(registries)
        }
    }
}

pub fn write_registries(app: &AppHandle, registries: &[RegistrySource]) -> Result<(), AppError> {
    let store = open_store(app)?;
    let value = serde_json::to_value(registries).map_err(|error| AppError::config(error))?;
    store.set("registries", value);
    store.save().map_err(|error| AppError::config(error))
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
    }

    #[test]
    fn node_input_never_serializes_secrets() {
        let input: NodeInput = serde_json::from_value(serde_json::json!({
            "name": "SSH 节点",
            "alias": "",
            "type": "remote",
            "connection": "ssh",
            "host": "10.0.0.9",
            "port": 22,
            "tls": false,
            "certPath": null,
            "username": "root",
            "authMethod": "password",
            "password": "never-serialize",
            "privateKey": "private-key-content",
            "passphrase": "passphrase"
        }))
        .unwrap();
        let value = serde_json::to_value(input).unwrap();
        assert!(value.get("password").is_none());
        assert!(value.get("privateKey").is_none());
        assert!(value.get("passphrase").is_none());
        assert!(value.get("clearCredential").is_none());
    }
}
