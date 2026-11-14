use crate::config::{NodeConnection, ShipNode, SshAuthMethod};
use crate::credential;
use crate::error::AppError;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use bollard::Docker;
use ssh2::{Channel, Session};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const HEARTBEAT_INTERVAL_SECONDS: usize = 30;

struct KnownHosts {
    path: Option<PathBuf>,
    entries: HashMap<String, String>,
}

static KNOWN_HOSTS: OnceLock<Mutex<KnownHosts>> = OnceLock::new();

fn known_hosts() -> &'static Mutex<KnownHosts> {
    KNOWN_HOSTS.get_or_init(|| {
        Mutex::new(KnownHosts {
            path: None,
            entries: HashMap::new(),
        })
    })
}

pub fn init_known_hosts(path: PathBuf) -> Result<(), AppError> {
    let entries = read_known_hosts(&path)?;
    let mut store = lock(known_hosts());
    store.path = Some(path);
    store.entries = entries;
    Ok(())
}

fn read_known_hosts(path: &Path) -> Result<HashMap<String, String>, AppError> {
    match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).map_err(|error| {
            AppError::config(format!(
                "SSH 主机密钥记录文件格式无效（{}）：{error}",
                path.display()
            ))
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(error) => Err(AppError::config(format!(
            "读取 SSH 主机密钥记录失败（{}）：{error}",
            path.display()
        ))),
    }
}

fn write_known_hosts(path: &Path, entries: &HashMap<String, String>) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| AppError::config(format!("无法创建 SSH 主机密钥记录目录：{error}")))?;
    }
    let content = serde_json::to_string_pretty(entries)
        .map_err(|error| AppError::config(format!("整理 SSH 主机密钥记录失败：{error}")))?;
    std::fs::write(path, content).map_err(|error| {
        AppError::config(format!(
            "无法写入 SSH 主机密钥记录（{}）：{error}",
            path.display()
        ))
    })
}

fn ssh_port(node: &ShipNode) -> u16 {
    if node.port == 0 {
        22
    } else {
        node.port
    }
}

fn host_key_identity(session: &Session) -> Result<String, AppError> {
    let (key, key_type) = session
        .host_key()
        .ok_or_else(|| AppError::host_key("SSH 服务端没有返回主机公钥，无法确认目标身份。"))?;
    Ok(format!("{key_type:?}:{}", BASE64.encode(key)))
}

enum HostKeyState {
    Matched,
    Recorded,
    Changed,
}

fn remember_host_key(
    entries: &mut HashMap<String, String>,
    entry: &str,
    observed: &str,
) -> HostKeyState {
    match entries.get(entry) {
        Some(known) if known == observed => HostKeyState::Matched,
        Some(_) => HostKeyState::Changed,
        None => {
            entries.insert(entry.to_string(), observed.to_string());
            HostKeyState::Recorded
        }
    }
}

fn verify_host_key(session: &Session, node: &ShipNode) -> Result<(), AppError> {
    let observed = host_key_identity(session)?;
    let entry = format!("{}:{}", node.host.trim().to_lowercase(), ssh_port(node));
    let mut store = lock(known_hosts());
    match remember_host_key(&mut store.entries, &entry, &observed) {
        HostKeyState::Matched => Ok(()),
        HostKeyState::Changed => {
            let location = store.path.as_ref().map_or_else(
                || "known_hosts.json".to_string(),
                |path| path.display().to_string(),
            );
            Err(AppError::host_key(format!(
                "目标舰船 {entry} 的主机密钥与首次连接记录不一致。记录位置：{location}。"
            )))
        }
        HostKeyState::Recorded => {
            let path = store.path.clone();
            let entries = store.entries.clone();
            drop(store);
            if let Some(path) = path {
                write_known_hosts(&path, &entries)?;
            }
            Ok(())
        }
    }
}

struct TunnelShared {
    node: ShipNode,
    shutdown: AtomicBool,
    healthy: AtomicBool,
    control_session: Mutex<Option<Session>>,
}

pub struct SshTunnel {
    shared: Arc<TunnelShared>,
    endpoint: LocalEndpoint,
}

impl SshTunnel {
    pub fn start(node: &ShipNode) -> Result<Arc<Self>, AppError> {
        if node.connection != NodeConnection::Ssh {
            return Err(AppError::validation("该舰船不是 SSH 连接。"));
        }
        let (endpoint, listener) = LocalEndpoint::bind()?;
        let control_session = connect_session(node)?;
        let mut channel = open_remote_socket(&control_session, node)?;
        let _ = channel.close();

        let shared = Arc::new(TunnelShared {
            node: node.clone(),
            shutdown: AtomicBool::new(false),
            healthy: AtomicBool::new(true),
            control_session: Mutex::new(Some(control_session)),
        });
        spawn_accept_loop(listener, Arc::clone(&shared));
        spawn_heartbeat(Arc::clone(&shared));
        Ok(Arc::new(Self { shared, endpoint }))
    }

    pub fn is_healthy(&self) -> bool {
        self.shared.healthy.load(Ordering::Acquire) && !self.shared.shutdown.load(Ordering::Acquire)
    }

    pub fn docker_client(&self) -> Result<Docker, AppError> {
        self.endpoint.docker_client()
    }

    pub fn docker_host(&self) -> Option<String> {
        Some(self.endpoint.docker_host())
    }

    pub fn shutdown(&self) {
        self.shared.shutdown.store(true, Ordering::Release);
        self.shared.healthy.store(false, Ordering::Release);
        let session = lock(&self.shared.control_session).take();
        if let Some(session) = session {
            let _ = session.disconnect(None, "ShipWiz 正在关闭连接", None);
        }
    }
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn connect_session(node: &ShipNode) -> Result<Session, AppError> {
    let host = nonempty(&node.host, "SSH 主机地址不能为空。")?;
    let port = ssh_port(node);
    let username = nonempty(
        node.username.as_deref().unwrap_or_default(),
        "SSH 用户名不能为空。",
    )?;
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|error| AppError::ssh(format!("无法解析 SSH 主机地址：{error}")))?;
    let mut last_error = None;
    let mut tcp = None;
    for address in addresses {
        match TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) {
            Ok(stream) => {
                tcp = Some(stream);
                break;
            }
            Err(error) => last_error = Some(error),
        }
    }
    let tcp = tcp.ok_or_else(|| {
        AppError::ssh(format!(
            "无法连接 SSH 服务：{}",
            last_error.map_or_else(|| "主机不可达".to_string(), |error| error.to_string())
        ))
    })?;
    tcp.set_nodelay(true).ok();
    tcp.set_read_timeout(Some(CONNECT_TIMEOUT)).ok();
    tcp.set_write_timeout(Some(CONNECT_TIMEOUT)).ok();

    let mut session =
        Session::new().map_err(|error| AppError::ssh(format!("初始化 SSH 会话失败：{error}")))?;
    session.set_timeout(CONNECT_TIMEOUT.as_millis() as u32);
    session.set_tcp_stream(tcp);
    session
        .handshake()
        .map_err(|error| AppError::ssh(format!("SSH 握手失败：{error}")))?;
    verify_host_key(&session, node)?;
    authenticate(&session, node, username)?;
    if !session.authenticated() {
        return Err(AppError::ssh("SSH 服务未确认认证成功。"));
    }
    session.set_keepalive(true, HEARTBEAT_INTERVAL_SECONDS as u32);
    Ok(session)
}

fn authenticate(session: &Session, node: &ShipNode, username: &str) -> Result<(), AppError> {
    let method = node
        .auth_method
        .ok_or_else(|| AppError::validation("SSH 节点缺少认证方式。"))?;
    let credentials = node
        .credential_id
        .as_deref()
        .and_then(|id| credential::load(id).ok())
        .unwrap_or_default();
    match method {
        SshAuthMethod::Password => {
            let password = credentials
                .password
                .or_else(|| env_secret("SHIPWIZ_SSH_PASSWORD"))
                .ok_or_else(|| {
                    AppError::credential(
                        "系统密钥库中没有 SSH 密码，请编辑节点并重新输入，或设置 SHIPWIZ_SSH_PASSWORD 环境变量。",
                    )
                })?;
            session
                .userauth_password(username, &password)
                .map_err(|error| {
                    AppError::ssh(format!("SSH 密码认证失败，请检查用户名和密码：{error}"))
                })
        }
        SshAuthMethod::PrivateKey => {
            let passphrase = credentials
                .passphrase
                .or_else(|| env_secret("SHIPWIZ_SSH_PASSPHRASE"));
            let private_key = credentials
                .private_key
                .or_else(|| env_secret("SHIPWIZ_SSH_PRIVATE_KEY"));
            if let Some(private_key) = private_key {
                session
                    .userauth_pubkey_memory(username, None, &private_key, passphrase.as_deref())
                    .map_err(|error| {
                        AppError::ssh(format!("SSH 私钥认证失败，请检查私钥和口令：{error}"))
                    })
            } else {
                let path = node
                    .private_key_path
                    .as_deref()
                    .ok_or_else(|| AppError::validation("SSH 私钥认证缺少私钥路径或私钥内容。"))?;
                session
                    .userauth_pubkey_file(username, None, Path::new(path), passphrase.as_deref())
                    .map_err(|error| {
                        AppError::ssh(format!("SSH 私钥文件认证失败，请检查路径和口令：{error}"))
                    })
            }
        }
    }
}

fn env_secret(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn open_remote_socket(session: &Session, node: &ShipNode) -> Result<Channel, AppError> {
    let socket = node
        .remote_socket
        .as_deref()
        .unwrap_or("/var/run/docker.sock");
    session
        .channel_direct_streamlocal(socket, None)
        .map_err(|error| {
            AppError::ssh(format!(
                "无法通过 SSH 打开远端 Docker Socket（{socket}）：{error}"
            ))
        })
}

fn nonempty<'a>(value: &'a str, message: &str) -> Result<&'a str, AppError> {
    let value = value.trim();
    if value.is_empty() {
        Err(AppError::validation(message))
    } else {
        Ok(value)
    }
}

fn spawn_heartbeat(shared: Arc<TunnelShared>) {
    thread::spawn(move || loop {
        for _ in 0..HEARTBEAT_INTERVAL_SECONDS {
            if shared.shutdown.load(Ordering::Acquire) {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }
        let result = {
            let session = lock(&shared.control_session);
            session.as_ref().map(|session| session.keepalive_send())
        };
        match result {
            Some(Ok(_)) if !shared.shutdown.load(Ordering::Acquire) => {}
            Some(Ok(_)) => return,
            Some(Err(_)) | None => {
                shared.healthy.store(false, Ordering::Release);
                return;
            }
        }
    });
}

fn spawn_connection<S>(stream: S, shared: Arc<TunnelShared>)
where
    S: Read + Write + Send + 'static,
{
    thread::spawn(move || {
        let session = match connect_session(&shared.node) {
            Ok(session) => session,
            Err(_) => {
                shared.healthy.store(false, Ordering::Release);
                return;
            }
        };
        let channel = match open_remote_socket(&session, &shared.node) {
            Ok(channel) => channel,
            Err(_) => {
                shared.healthy.store(false, Ordering::Release);
                return;
            }
        };
        session.set_blocking(false);
        bridge(stream, channel, &shared.shutdown);
    });
}

fn bridge<S>(mut local: S, mut channel: Channel, shutdown: &AtomicBool)
where
    S: Read + Write,
{
    let mut local_open = true;
    let mut remote_open = true;
    let mut eof_sent = false;
    let mut to_remote = PendingBuffer::default();
    let mut to_local = PendingBuffer::default();
    let mut buffer = [0_u8; 16 * 1024];

    while !shutdown.load(Ordering::Acquire) {
        let mut progressed = false;
        if local_open && to_remote.is_empty() {
            match local.read(&mut buffer) {
                Ok(0) => local_open = false,
                Ok(count) => {
                    to_remote.set(&buffer[..count]);
                    progressed = true;
                }
                Err(error) if is_would_block(&error) => {}
                Err(_) => local_open = false,
            }
        }
        if !to_remote.is_empty() {
            match channel.write(to_remote.remaining()) {
                Ok(0) => break,
                Ok(count) => {
                    to_remote.advance(count);
                    progressed = true;
                }
                Err(error) if is_would_block(&error) => {}
                Err(_) => break,
            }
        }
        if !local_open && to_remote.is_empty() && !eof_sent {
            let _ = channel.send_eof();
            eof_sent = true;
        }

        if remote_open && to_local.is_empty() {
            match channel.read(&mut buffer) {
                Ok(0) => remote_open = false,
                Ok(count) => {
                    to_local.set(&buffer[..count]);
                    progressed = true;
                }
                Err(error) if is_would_block(&error) => {}
                Err(_) => break,
            }
        }
        if !to_local.is_empty() {
            match local.write(to_local.remaining()) {
                Ok(0) => break,
                Ok(count) => {
                    to_local.advance(count);
                    progressed = true;
                }
                Err(error) if is_would_block(&error) => {}
                Err(_) => break,
            }
        }
        if !remote_open && to_local.is_empty() {
            break;
        }
        if !progressed {
            thread::sleep(Duration::from_millis(2));
        }
    }
    let _ = channel.close();
}

#[derive(Default)]
struct PendingBuffer {
    bytes: Vec<u8>,
    offset: usize,
}

impl PendingBuffer {
    fn set(&mut self, bytes: &[u8]) {
        self.bytes.clear();
        self.bytes.extend_from_slice(bytes);
        self.offset = 0;
    }

    fn remaining(&self) -> &[u8] {
        &self.bytes[self.offset..]
    }

    fn advance(&mut self, count: usize) {
        self.offset += count;
        if self.offset == self.bytes.len() {
            self.bytes.clear();
            self.offset = 0;
        }
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn is_would_block(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

enum LocalEndpoint {
    #[cfg(windows)]
    NamedPipe { path: String },
    #[cfg(unix)]
    Unix { socket: PathBuf, directory: PathBuf },
}

impl LocalEndpoint {
    #[cfg(windows)]
    fn bind() -> Result<(Self, LocalListener), AppError> {
        let path = format!(r"\\.\pipe\shipwiz_{}", Uuid::new_v4().simple());
        let server = create_pipe_server(&path)?;
        Ok((
            Self::NamedPipe { path: path.clone() },
            LocalListener { path, server },
        ))
    }

    #[cfg(unix)]
    fn bind() -> Result<(Self, LocalListener), AppError> {
        use std::os::unix::fs::DirBuilderExt;

        let directory = std::env::temp_dir().join(format!("shipwiz-{}", Uuid::new_v4().simple()));
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder
            .create(&directory)
            .map_err(|error| AppError::ssh(format!("创建临时 Socket 目录失败：{error}")))?;
        let socket = directory.join("docker.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket)
            .map_err(|error| AppError::ssh(format!("创建本机 Unix Socket 转发失败：{error}")))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| AppError::ssh(format!("配置 Unix Socket 转发失败：{error}")))?;
        Ok((
            Self::Unix {
                socket: socket.clone(),
                directory: directory.clone(),
            },
            LocalListener { listener },
        ))
    }

    fn docker_host(&self) -> String {
        match self {
            #[cfg(windows)]
            Self::NamedPipe { path } => format!("npipe://{}", path.replace('\\', "/")),
            #[cfg(unix)]
            Self::Unix { socket, .. } => format!("unix://{}", socket.display()),
        }
    }

    fn docker_client(&self) -> Result<Docker, AppError> {
        match self {
            #[cfg(windows)]
            Self::NamedPipe { path } => {
                Docker::connect_with_named_pipe(path, 8, bollard::API_DEFAULT_VERSION)
                    .map_err(AppError::docker)
            }
            #[cfg(unix)]
            Self::Unix { socket, .. } => Docker::connect_with_unix(
                &format!("unix://{}", socket.display()),
                8,
                bollard::API_DEFAULT_VERSION,
            )
            .map_err(AppError::docker),
        }
    }
}

#[cfg(unix)]
impl Drop for LocalEndpoint {
    fn drop(&mut self) {
        if let Self::Unix { socket, directory } = self {
            let _ = std::fs::remove_file(socket);
            let _ = std::fs::remove_dir(directory);
        }
    }
}

#[cfg(windows)]
struct LocalListener {
    path: String,
    server: named_pipe::ConnectingServer,
}

#[cfg(unix)]
struct LocalListener {
    listener: std::os::unix::net::UnixListener,
}

#[cfg(windows)]
fn create_pipe_server(path: &str) -> Result<named_pipe::ConnectingServer, AppError> {
    named_pipe::PipeOptions::new(path)
        .first(false)
        .single()
        .map_err(|error| AppError::ssh(format!("创建本机 Named Pipe 转发失败：{error}")))
}

#[cfg(windows)]
fn spawn_accept_loop(listener: LocalListener, shared: Arc<TunnelShared>) {
    thread::spawn(move || {
        let LocalListener { path, mut server } = listener;
        loop {
            if shared.shutdown.load(Ordering::Acquire) {
                return;
            }
            match server.wait_ms(250) {
                Ok(Ok(mut connected)) => {
                    connected.set_read_timeout(Some(Duration::ZERO));
                    connected.set_write_timeout(Some(Duration::ZERO));
                    spawn_connection(connected, Arc::clone(&shared));
                    if shared.shutdown.load(Ordering::Acquire) {
                        return;
                    }
                    server = match create_pipe_server(&path) {
                        Ok(server) => server,
                        Err(_) => {
                            shared.healthy.store(false, Ordering::Release);
                            return;
                        }
                    };
                }
                Ok(Err(connecting)) => server = connecting,
                Err(_) => {
                    if !shared.shutdown.load(Ordering::Acquire) {
                        shared.healthy.store(false, Ordering::Release);
                    }
                    return;
                }
            }
        }
    });
}

#[cfg(unix)]
fn spawn_accept_loop(listener: LocalListener, shared: Arc<TunnelShared>) {
    thread::spawn(move || {
        let LocalListener { listener } = listener;
        while !shared.shutdown.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(true).ok();
                    spawn_connection(stream, Arc::clone(&shared));
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(_) => {
                    shared.healthy.store(false, Ordering::Release);
                    return;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_key_trust_records_once_and_rejects_changes() {
        let mut entries = HashMap::new();
        assert!(matches!(
            remember_host_key(&mut entries, "ship:22", "ed25519:AAAA"),
            HostKeyState::Recorded
        ));
        assert!(matches!(
            remember_host_key(&mut entries, "ship:22", "ed25519:AAAA"),
            HostKeyState::Matched
        ));
        assert!(matches!(
            remember_host_key(&mut entries, "ship:22", "ed25519:BBBB"),
            HostKeyState::Changed
        ));
    }

    #[test]
    fn pending_buffer_tracks_partial_writes() {
        let mut buffer = PendingBuffer::default();
        buffer.set(b"abcdef");
        buffer.advance(2);
        assert_eq!(buffer.remaining(), b"cdef");
        buffer.advance(4);
        assert!(buffer.is_empty());
        buffer.set(b"xy");
        assert_eq!(buffer.remaining(), b"xy");
    }
}
