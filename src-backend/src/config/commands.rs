use super::{
    load_registries, load_state, write_registries, write_state, FleetState, NodeConnection,
    NodeInput, NodeStatus, NodeType, RegistryInput, RegistrySource, ShipNode, SshAuthMethod,
};
use crate::credential::{self, StoredCredential};
use crate::docker;
use crate::error::{respond, ApiResponse, AppError};
use crate::utils::timestamp_ms;
use tauri::AppHandle;
use uuid::Uuid;

struct PreparedNode {
    node: ShipNode,
    save_credential: Option<(String, StoredCredential)>,
    delete_credentials: Vec<String>,
}

#[tauri::command]
pub async fn get_fleet_state(app: AppHandle) -> ApiResponse<FleetState> {
    respond(load_state(&app))
}

#[tauri::command]
pub async fn create_node(app: AppHandle, input: NodeInput) -> ApiResponse<FleetState> {
    respond((|| {
        let prepared = prepare_node(input, None)?;
        if let Some((id, credential)) = &prepared.save_credential {
            credential::save(id, credential)?;
        }
        let mut state = load_state(&app)?;
        state.nodes.push(prepared.node);
        write_state(&app, &state)?;
        Ok(state)
    })())
}

#[tauri::command]
pub async fn update_node(app: AppHandle, id: String, input: NodeInput) -> ApiResponse<FleetState> {
    respond((|| {
        let mut state = load_state(&app)?;
        let existing = state
            .nodes
            .iter()
            .find(|node| node.id == id)
            .cloned()
            .ok_or_else(|| AppError::validation("目标舰船不存在或已被移出舰队。"))?;
        let prepared = prepare_node(input, Some(&existing))?;
        if let Some((credential_id, credential)) = &prepared.save_credential {
            credential::save(credential_id, credential)?;
        }
        let node = find_node_mut(&mut state, &id)?;
        *node = prepared.node;
        write_state(&app, &state)?;
        docker::disconnect_node(&id);
        for credential_id in prepared.delete_credentials {
            credential::delete(&credential_id)?;
        }
        Ok(state)
    })())
}

#[tauri::command]
pub async fn remove_node(app: AppHandle, id: String) -> ApiResponse<FleetState> {
    respond((|| {
        let mut state = load_state(&app)?;
        let original_len = state.nodes.len();
        let credential_id = state
            .nodes
            .iter()
            .find(|node| node.id == id)
            .and_then(|node| node.credential_id.clone());
        state.nodes.retain(|node| node.id != id);
        if state.nodes.len() == original_len {
            return Err(AppError::validation("目标舰船不存在或已被移出舰队。"));
        }
        if state.current_node_id.as_deref() == Some(id.as_str()) {
            state.current_node_id = state.nodes.first().map(|node| node.id.clone());
        }
        if let Some(credential_id) = credential_id {
            credential::delete(&credential_id)?;
        }
        write_state(&app, &state)?;
        docker::disconnect_node(&id);
        Ok(state)
    })())
}

#[tauri::command]
pub async fn set_current_node(app: AppHandle, id: String) -> ApiResponse<FleetState> {
    respond((|| {
        let mut state = load_state(&app)?;
        if !state.nodes.iter().any(|node| node.id == id) {
            return Err(AppError::validation("无法切换到不存在的舰船。"));
        }
        state.current_node_id = Some(id);
        write_state(&app, &state)?;
        Ok(state)
    })())
}

#[tauri::command]
pub async fn test_node(app: AppHandle, id: String) -> ApiResponse<ShipNode> {
    let result = async {
        let mut state = load_state(&app)?;
        let index = state
            .nodes
            .iter()
            .position(|node| node.id == id)
            .ok_or_else(|| AppError::validation("目标舰船不存在或已被移出舰队。"))?;

        docker::disconnect_node(&id);
        match docker::check_connection(&state.nodes[index]).await {
            Ok(()) => {
                state.nodes[index].status = NodeStatus::Online;
                state.nodes[index].last_success_time = Some(timestamp_ms());
                write_state(&app, &state)?;
                Ok(state.nodes[index].clone())
            }
            Err(connection_error) => {
                state.nodes[index].status = NodeStatus::Offline;
                write_state(&app, &state)?;
                Err(connection_error)
            }
        }
    }
    .await;
    respond(result)
}

#[tauri::command]
pub async fn list_registries(app: AppHandle) -> ApiResponse<Vec<RegistrySource>> {
    respond(load_registries(&app))
}

#[tauri::command]
pub async fn save_registry(
    app: AppHandle,
    input: RegistryInput,
    id: Option<String>,
) -> ApiResponse<Vec<RegistrySource>> {
    respond((|| {
        let (name, url) = normalize_registry(input.name, input.url)?;
        let mut registries = load_registries(&app)?;
        if id.as_deref() == Some("docker-hub") {
            return Err(AppError::validation(
                "官方补给港不可编辑，可新增自定义补给港。",
            ));
        }
        let selected_id = if let Some(id) = id {
            let registry = registries
                .iter_mut()
                .find(|registry| registry.id == id)
                .ok_or_else(|| AppError::validation("目标补给港不存在或已被移除。"))?;
            registry.name = name;
            registry.url = url;
            id
        } else {
            let id = Uuid::new_v4().to_string();
            registries.push(RegistrySource {
                id: id.clone(),
                name,
                url,
                is_default: false,
            });
            id
        };
        ensure_default_registry(
            &mut registries,
            input.is_default.then_some(selected_id.as_str()),
        )?;
        write_registries(&app, &registries)?;
        Ok(registries)
    })())
}

#[tauri::command]
pub async fn remove_registry(app: AppHandle, id: String) -> ApiResponse<Vec<RegistrySource>> {
    respond((|| {
        if id == "docker-hub" {
            return Err(AppError::validation("官方补给港不可移除。"));
        }
        let mut registries = load_registries(&app)?;
        let original_len = registries.len();
        registries.retain(|registry| registry.id != id);
        if registries.len() == original_len {
            return Err(AppError::validation("目标补给港不存在或已被移除。"));
        }
        ensure_default_registry(&mut registries, None)?;
        write_registries(&app, &registries)?;
        Ok(registries)
    })())
}

#[tauri::command]
pub async fn set_default_registry(app: AppHandle, id: String) -> ApiResponse<Vec<RegistrySource>> {
    respond((|| {
        let mut registries = load_registries(&app)?;
        if !registries.iter().any(|registry| registry.id == id) {
            return Err(AppError::validation("目标补给港不存在或已被移除。"));
        }
        for registry in &mut registries {
            registry.is_default = registry.id == id;
        }
        write_registries(&app, &registries)?;
        Ok(registries)
    })())
}

fn prepare_node(input: NodeInput, existing: Option<&ShipNode>) -> Result<PreparedNode, AppError> {
    let input = normalize(input)?;
    let previous_credential_id = existing.and_then(|node| node.credential_id.clone());
    let can_reuse = existing.is_some_and(|node| {
        node.connection == NodeConnection::Ssh && node.auth_method == input.auth_method
    });
    let mut credential =
        if input.connection == NodeConnection::Ssh && can_reuse && !input.clear_credential {
            if let Some(id) = previous_credential_id.as_deref() {
                credential::load(id)?
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
    match input.auth_method {
        Some(SshAuthMethod::Password)
            if credential.password.as_deref().map_or(true, str::is_empty) =>
        {
            Err(AppError::validation("SSH 密码不能为空。"))
        }
        Some(SshAuthMethod::PrivateKey)
            if credential
                .private_key
                .as_deref()
                .map_or(true, str::is_empty)
                && input.private_key_path.is_none() =>
        {
            Err(AppError::validation(
                "SSH 私钥认证必须填写私钥文件路径或粘贴私钥内容。",
            ))
        }
        _ => Ok(()),
    }
}

fn normalize_registry(name: String, url: String) -> Result<(String, String), AppError> {
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

fn ensure_default_registry(
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

fn find_node_mut<'a>(state: &'a mut FleetState, id: &str) -> Result<&'a mut ShipNode, AppError> {
    state
        .nodes
        .iter_mut()
        .find(|node| node.id == id)
        .ok_or_else(|| AppError::validation("目标舰船不存在或已被移出舰队。"))
}

fn normalize(mut input: NodeInput) -> Result<NodeInput, AppError> {
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
            "authMethod": "password"
        }))
        .unwrap();
        let input = normalize(input).unwrap();
        assert_eq!(input.port, 22);
        assert_eq!(input.remote_socket.as_deref(), Some("/var/run/docker.sock"));
        assert!(!input.tls);
        assert!(input.cert_path.is_none());
    }
}
