use super::{
    ensure_default_registry, find_node_mut, load_registries, load_state, normalize_registry,
    prepare_node, write_registries, write_state, FleetState, NodeInput, NodeStatus, RegistryInput,
    RegistrySource, ShipNode,
};
use crate::credential;
use crate::docker;
use crate::error::{respond, ApiResponse, AppError};
use crate::utils::timestamp_ms;
use uuid::Uuid;

#[tauri::command]
pub async fn get_fleet_state() -> ApiResponse<FleetState> {
    respond(load_state())
}

#[tauri::command]
pub async fn create_node(input: NodeInput) -> ApiResponse<FleetState> {
    respond((|| {
        let prepared = prepare_node(input, None)?;
        if let Some((id, credential)) = &prepared.save_credential {
            credential::save(id, credential)?;
        }
        let mut state = load_state()?;
        state.nodes.push(prepared.node);
        write_state(&state)?;
        Ok(state)
    })())
}

#[tauri::command]
pub async fn update_node(id: String, input: NodeInput) -> ApiResponse<FleetState> {
    respond((|| {
        let mut state = load_state()?;
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
        write_state(&state)?;
        for credential_id in prepared.delete_credentials {
            credential::delete(&credential_id)?;
        }
        Ok(state)
    })())
}

#[tauri::command]
pub async fn remove_node(id: String) -> ApiResponse<FleetState> {
    respond((|| {
        let mut state = load_state()?;
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
        write_state(&state)?;
        docker::disconnect_node(&id);
        Ok(state)
    })())
}

#[tauri::command]
pub async fn set_current_node(id: String) -> ApiResponse<FleetState> {
    respond((|| {
        let mut state = load_state()?;
        if !state.nodes.iter().any(|node| node.id == id) {
            return Err(AppError::validation("无法切换到不存在的舰船。"));
        }
        state.current_node_id = Some(id);
        write_state(&state)?;
        Ok(state)
    })())
}

#[tauri::command]
pub async fn test_node(id: String) -> ApiResponse<ShipNode> {
    let result = async {
        let mut state = load_state()?;
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
                write_state(&state)?;
                Ok(state.nodes[index].clone())
            }
            Err(connection_error) => {
                state.nodes[index].status = NodeStatus::Offline;
                write_state(&state)?;
                Err(connection_error)
            }
        }
    }
    .await;
    respond(result)
}

#[tauri::command]
pub async fn list_registries() -> ApiResponse<Vec<RegistrySource>> {
    respond(load_registries())
}

#[tauri::command]
pub async fn save_registry(
    input: RegistryInput,
    id: Option<String>,
) -> ApiResponse<Vec<RegistrySource>> {
    respond((|| {
        let (name, url) = normalize_registry(input.name, input.url)?;
        let mut registries = load_registries()?;
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
        write_registries(&registries)?;
        Ok(registries)
    })())
}

#[tauri::command]
pub async fn remove_registry(id: String) -> ApiResponse<Vec<RegistrySource>> {
    respond((|| {
        if id == "docker-hub" {
            return Err(AppError::validation("官方补给港不可移除。"));
        }
        let mut registries = load_registries()?;
        let original_len = registries.len();
        registries.retain(|registry| registry.id != id);
        if registries.len() == original_len {
            return Err(AppError::validation("目标补给港不存在或已被移除。"));
        }
        ensure_default_registry(&mut registries, None)?;
        write_registries(&registries)?;
        Ok(registries)
    })())
}

#[tauri::command]
pub async fn set_default_registry(id: String) -> ApiResponse<Vec<RegistrySource>> {
    respond((|| {
        let mut registries = load_registries()?;
        if !registries.iter().any(|registry| registry.id == id) {
            return Err(AppError::validation("目标补给港不存在或已被移除。"));
        }
        for registry in &mut registries {
            registry.is_default = registry.id == id;
        }
        write_registries(&registries)?;
        Ok(registries)
    })())
}
