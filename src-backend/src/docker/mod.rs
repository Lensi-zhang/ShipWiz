pub mod container;
pub mod image;

use crate::config::{NodeConnection, ShipNode};
use crate::error::AppError;
use crate::ssh::SshTunnel;
use bollard::Docker;
use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::time::timeout;

const CONNECT_TIMEOUT_SECONDS: u64 = 8;

#[derive(Clone)]
pub struct DockerLease {
    docker: Docker,
    _tunnel: Option<Arc<SshTunnel>>,
}

impl DockerLease {
    pub fn local_docker_host(&self) -> Option<String> {
        self._tunnel
            .as_ref()
            .and_then(|tunnel| tunnel.docker_host())
    }
}

impl Deref for DockerLease {
    type Target = Docker;

    fn deref(&self) -> &Self::Target {
        &self.docker
    }
}

struct TunnelEntry {
    node: ShipNode,
    tunnel: Arc<SshTunnel>,
}

static TUNNELS: OnceLock<Mutex<HashMap<String, TunnelEntry>>> = OnceLock::new();

fn tunnels() -> &'static Mutex<HashMap<String, TunnelEntry>> {
    TUNNELS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_tunnels() -> std::sync::MutexGuard<'static, HashMap<String, TunnelEntry>> {
    tunnels()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub async fn check_connection(node: &ShipNode) -> Result<(), AppError> {
    let docker = connect(node).await?;
    timeout(Duration::from_secs(CONNECT_TIMEOUT_SECONDS), async {
        docker.ping().await.map_err(AppError::docker)?;
        docker.version().await.map_err(AppError::docker)?;
        Ok(())
    })
    .await
    .map_err(|_| AppError::docker("连接引擎超时，请检查服务状态与网络。"))?
}

pub(crate) async fn connect(node: &ShipNode) -> Result<DockerLease, AppError> {
    match node.connection {
        NodeConnection::Local => Docker::connect_with_local_defaults()
            .map(|docker| DockerLease {
                docker,
                _tunnel: None,
            })
            .map_err(AppError::docker),
        NodeConnection::Tls => {
            if !node.tls {
                return Err(AppError::validation(
                    "远程舰船必须使用 TLS，禁止未加密的 TCP 连接。",
                ));
            }
            let cert_dir = node
                .cert_path
                .as_deref()
                .ok_or_else(|| AppError::validation("TLS 节点缺少证书目录，请编辑节点配置。"))?;
            let address = format!("{}:{}", node.host, node.port);
            let cert_dir = Path::new(cert_dir);
            Docker::connect_with_ssl(
                &address,
                &cert_dir.join("key.pem"),
                &cert_dir.join("cert.pem"),
                &cert_dir.join("ca.pem"),
                CONNECT_TIMEOUT_SECONDS,
                bollard::API_DEFAULT_VERSION,
            )
            .map(|docker| DockerLease {
                docker,
                _tunnel: None,
            })
            .map_err(AppError::docker)
        }
        NodeConnection::Ssh => {
            let tunnel = ssh_tunnel(node).await?;
            let docker = tunnel.docker_client()?;
            Ok(DockerLease {
                docker,
                _tunnel: Some(tunnel),
            })
        }
    }
}

async fn ssh_tunnel(node: &ShipNode) -> Result<Arc<SshTunnel>, AppError> {
    let stale = {
        let mut entries = lock_tunnels();
        if let Some(entry) = entries.get(&node.id) {
            if entry.node.same_connection(node) && entry.tunnel.is_healthy() {
                return Ok(Arc::clone(&entry.tunnel));
            }
        }
        entries.remove(&node.id)
    };
    if let Some(entry) = stale {
        entry.tunnel.shutdown();
    }

    let node_for_task = node.clone();
    let new_tunnel = timeout(
        Duration::from_secs(CONNECT_TIMEOUT_SECONDS + 2),
        tokio::task::spawn_blocking(move || SshTunnel::start(&node_for_task)),
    )
    .await
    .map_err(|_| AppError::ssh("建立 SSH 隧道超时，请检查网络后手动重试。"))?
    .map_err(|_| AppError::ssh("SSH 隧道任务被意外取消，请手动重试。"))??;

    let previous = {
        let mut entries = lock_tunnels();
        if let Some(entry) = entries.get(&node.id) {
            if entry.node.same_connection(node) && entry.tunnel.is_healthy() {
                let tunnel = Arc::clone(&entry.tunnel);
                drop(entries);
                new_tunnel.shutdown();
                return Ok(tunnel);
            }
        }
        entries.insert(
            node.id.clone(),
            TunnelEntry {
                node: node.clone(),
                tunnel: Arc::clone(&new_tunnel),
            },
        )
    };
    if let Some(entry) = previous {
        entry.tunnel.shutdown();
    }
    Ok(new_tunnel)
}

pub fn disconnect_node(node_id: &str) {
    let entry = lock_tunnels().remove(node_id);
    if let Some(entry) = entry {
        entry.tunnel.shutdown();
    }
}

#[cfg(feature = "desktop")]
pub fn shutdown_connections() {
    let entries = {
        let mut tunnels = lock_tunnels();
        tunnels.drain().map(|(_, entry)| entry).collect::<Vec<_>>()
    };
    for entry in entries {
        entry.tunnel.shutdown();
    }
}
