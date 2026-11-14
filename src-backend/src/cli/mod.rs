use crate::compose;
use crate::config::{
    self, FleetState, NodeConnection, NodeInput, NodeStatus, NodeType, RegistrySource, ShipNode,
    SshAuthMethod,
};
use crate::credential::{self, StoredCredential};
use crate::docker::{self, container, image};
use crate::error::AppError;
use crate::ssh;
use crate::utils::timestamp_ms;
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(
    name = "shipwiz-cli",
    version,
    about = "ShipWiz 命令行舰队管理器",
    long_about = "通过本机、TLS 或 SSH 通道管理 Docker 引擎，与 ShipWiz 桌面端共用 ~/.shipwiz/config.json。"
)]
struct Cli {
    /// 目标节点 ID 或名称，默认使用当前节点
    #[arg(short = 'n', long, global = true, value_name = "NODE")]
    node: Option<String>,
    /// 以 JSON 输出结果
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// 舰队节点管理
    Node {
        #[command(subcommand)]
        action: NodeAction,
    },
    /// 列出集装箱（容器）
    Ps,
    /// 启动集装箱
    Start { container: String },
    /// 停止集装箱
    Stop { container: String },
    /// 重启集装箱
    Restart { container: String },
    /// 移除集装箱
    Rm {
        container: String,
        /// 强制移除运行中的集装箱
        #[arg(long)]
        force: bool,
    },
    /// 查看集装箱日志
    Logs {
        container: String,
        /// 显示的末尾行数
        #[arg(short = 't', long, default_value_t = 200)]
        tail: u32,
        /// 持续跟踪日志
        #[arg(short = 'f', long)]
        follow: bool,
    },
    /// 在集装箱内执行命令
    Exec {
        container: String,
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },
    /// 创建并可选启动集装箱
    Create(CreateArgs),
    /// 图纸（镜像）管理
    Image {
        #[command(subcommand)]
        action: ImageAction,
    },
    /// 编队（Compose）管理
    Compose {
        #[command(subcommand)]
        action: ComposeAction,
    },
    /// 补给港（镜像仓库）管理
    Registry {
        #[command(subcommand)]
        action: RegistryAction,
    },
    /// 舰队概览
    Monitor {
        /// 持续刷新
        #[arg(long)]
        watch: bool,
        /// 刷新间隔（秒）
        #[arg(long, default_value_t = 3)]
        interval: u64,
    },
    /// 显示版本与配置路径
    Info,
}

#[derive(Subcommand, Debug)]
enum NodeAction {
    /// 列出舰队节点
    #[command(alias = "ls")]
    List,
    /// 查看节点详情
    Show { node: Option<String> },
    /// 新增节点
    Add(NodeArgs),
    /// 修改节点
    Edit {
        node: String,
        #[command(flatten)]
        args: NodeArgs,
    },
    /// 切换当前节点
    Use { node: String },
    /// 测试节点连接
    Test { node: Option<String> },
    /// 移除节点
    Rm { node: String },
}

#[derive(Args, Debug)]
struct NodeArgs {
    /// 舰船名称
    #[arg(long)]
    name: Option<String>,
    /// 备注说明
    #[arg(long)]
    alias: Option<String>,
    /// 节点类型
    #[arg(long = "type", value_enum)]
    node_type: Option<NodeTypeArg>,
    /// 连接方式
    #[arg(long, value_enum)]
    connection: Option<ConnectionArg>,
    /// 主机地址（IP 或域名）
    #[arg(long)]
    host: Option<String>,
    /// 服务端口（TLS 默认 2376，SSH 默认 22）
    #[arg(long)]
    port: Option<u16>,
    /// TLS 证书目录（含 ca.pem、cert.pem、key.pem）
    #[arg(long)]
    cert_path: Option<String>,
    /// SSH 用户名
    #[arg(long)]
    username: Option<String>,
    /// SSH 认证方式
    #[arg(long, value_enum)]
    auth: Option<AuthArg>,
    /// SSH 私钥文件路径
    #[arg(long)]
    private_key_path: Option<String>,
    /// 远端 Docker Socket
    #[arg(long)]
    remote_socket: Option<String>,
    /// SSH 密码（建议改用 SHIPWIZ_SSH_PASSWORD 环境变量）
    #[arg(long)]
    password: Option<String>,
    /// SSH 私钥内容（建议改用 SHIPWIZ_SSH_PRIVATE_KEY 环境变量）
    #[arg(long)]
    private_key: Option<String>,
    /// SSH 私钥口令（建议改用 SHIPWIZ_SSH_PASSPHRASE 环境变量）
    #[arg(long)]
    passphrase: Option<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum NodeTypeArg {
    Local,
    Remote,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ConnectionArg {
    Local,
    Tls,
    Ssh,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum AuthArg {
    Password,
    Key,
}

impl From<NodeTypeArg> for NodeType {
    fn from(value: NodeTypeArg) -> Self {
        match value {
            NodeTypeArg::Local => NodeType::Local,
            NodeTypeArg::Remote => NodeType::Remote,
        }
    }
}

impl From<ConnectionArg> for NodeConnection {
    fn from(value: ConnectionArg) -> Self {
        match value {
            ConnectionArg::Local => NodeConnection::Local,
            ConnectionArg::Tls => NodeConnection::Tls,
            ConnectionArg::Ssh => NodeConnection::Ssh,
        }
    }
}

impl From<AuthArg> for SshAuthMethod {
    fn from(value: AuthArg) -> Self {
        match value {
            AuthArg::Password => SshAuthMethod::Password,
            AuthArg::Key => SshAuthMethod::PrivateKey,
        }
    }
}

#[derive(Args, Debug)]
struct CreateArgs {
    /// 集装箱名称
    #[arg(long)]
    name: String,
    /// 图纸名称与标签
    #[arg(long)]
    image: String,
    /// 启动命令
    #[arg(long)]
    command: Option<String>,
    /// 环境变量 KEY=VALUE，可重复
    #[arg(long = "env")]
    env: Vec<String>,
    /// 端口映射 host:container[/tcp|/udp]，可重复
    #[arg(long = "port")]
    ports: Vec<String>,
    /// 挂载 source:target[:ro]，可重复
    #[arg(long = "volume")]
    volumes: Vec<String>,
    /// 重启策略：no、always、unless-stopped、on-failure
    #[arg(long, default_value = "no")]
    restart: String,
    /// 网络名称
    #[arg(long)]
    network: Option<String>,
    /// 退出后自动删除
    #[arg(long)]
    auto_remove: bool,
    /// 创建后立即启动
    #[arg(long)]
    start: bool,
}

#[derive(Subcommand, Debug)]
enum ImageAction {
    /// 列出图纸
    #[command(alias = "ls")]
    List,
    /// 拉取图纸
    Pull {
        reference: String,
        /// 指定补给港 ID
        #[arg(long)]
        registry: Option<String>,
    },
    /// 删除图纸
    #[command(alias = "rmi")]
    Rm {
        image: String,
        #[arg(long)]
        force: bool,
    },
    /// 搜索图纸
    Search { term: String },
    /// 导出图纸为 tar 归档
    Export {
        reference: String,
        destination: String,
    },
    /// 从 tar 归档导入图纸
    Import { source: String },
}

#[derive(Subcommand, Debug)]
enum ComposeAction {
    /// 列出已保存的编队
    #[command(alias = "ls")]
    List,
    /// 列出运行中的编队
    Ps,
    /// 启动编队
    Up { project: String },
    /// 停止编队
    Down { project: String },
    /// 查看编队日志
    Logs {
        project: String,
        #[arg(short = 't', long, default_value_t = 200)]
        tail: u32,
    },
    /// 保存编队 YAML
    Save {
        name: String,
        /// YAML 文件路径
        #[arg(long)]
        file: PathBuf,
        /// 覆盖已有编队 ID
        #[arg(long)]
        id: Option<String>,
    },
    /// 移除已保存的编队
    Rm { project: String },
    /// 校验编队 YAML（文件路径或 - 表示标准输入）
    Validate { file: String },
}

#[derive(Subcommand, Debug)]
enum RegistryAction {
    /// 列出补给港
    #[command(alias = "ls")]
    List,
    /// 新增补给港
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        url: String,
        /// 设为默认补给港
        #[arg(long)]
        default: bool,
    },
    /// 设为默认补给港
    Use { registry: String },
    /// 移除补给港
    Rm { registry: String },
}

pub async fn run() -> i32 {
    let cli = Cli::parse();
    if let Err(error) = prepare() {
        eprintln!("{}", error.message());
        return 1;
    }
    match dispatch(cli).await {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{}", error.message());
            1
        }
    }
}

fn prepare() -> Result<(), AppError> {
    let directory = config::config_directory()?;
    std::fs::create_dir_all(&directory).map_err(AppError::config)?;
    ssh::init_known_hosts(directory.join("known_hosts.json"))
}

async fn dispatch(cli: Cli) -> Result<(), AppError> {
    let json = cli.json;
    match cli.command {
        Command::Node { action } => handle_node(action, json).await,
        Command::Ps => list_containers(cli.node.as_deref(), json).await,
        Command::Start { container } => {
            let node = active_node(cli.node.as_deref())?;
            container::start_for_node(&node, &container).await?;
            action_done(json, "start", &node, &container)
        }
        Command::Stop { container } => {
            let node = active_node(cli.node.as_deref())?;
            container::stop_for_node(&node, &container).await?;
            action_done(json, "stop", &node, &container)
        }
        Command::Restart { container } => {
            let node = active_node(cli.node.as_deref())?;
            container::restart_for_node(&node, &container).await?;
            action_done(json, "restart", &node, &container)
        }
        Command::Rm { container, force } => {
            let node = active_node(cli.node.as_deref())?;
            container::remove_for_node(&node, &container, force).await?;
            action_done(json, "remove", &node, &container)
        }
        Command::Logs {
            container,
            tail,
            follow,
        } => {
            let node = active_node(cli.node.as_deref())?;
            if follow {
                container::stream_logs_for_node(&node, &container, tail, true, |line| {
                    print!("{line}");
                })
                .await
            } else {
                let output = container::logs_for_node(&node, &container, tail).await?;
                if json {
                    print_json(&serde_json::json!({
                        "node": node.name,
                        "container": container,
                        "logs": output,
                    }))
                } else {
                    print!("{output}");
                    Ok(())
                }
            }
        }
        Command::Exec { container, command } => {
            let node = active_node(cli.node.as_deref())?;
            let joined = command.join(" ");
            let result = container::exec_for_node(&node, &container, &joined).await?;
            if json {
                print_json(&result)
            } else {
                print!("{}", result.output);
                if let Some(code) = result.exit_code {
                    if code != 0 {
                        eprintln!("命令退出码：{code}");
                    }
                }
                Ok(())
            }
        }
        Command::Create(args) => create_container(cli.node.as_deref(), args, json).await,
        Command::Image { action } => handle_image(action, cli.node.as_deref(), json).await,
        Command::Compose { action } => handle_compose(action, cli.node.as_deref(), json).await,
        Command::Registry { action } => handle_registry(action, json),
        Command::Monitor { watch, interval } => monitor(json, watch, interval).await,
        Command::Info => info(json),
    }
}

async fn handle_node(action: NodeAction, json: bool) -> Result<(), AppError> {
    match action {
        NodeAction::List => {
            let state = config::load_state()?;
            if json {
                return print_json(&state);
            }
            let rows = state
                .nodes
                .iter()
                .map(|node| {
                    vec![
                        if state.current_node_id.as_deref() == Some(node.id.as_str()) {
                            "*".to_string()
                        } else {
                            String::new()
                        },
                        node.id.chars().take(8).collect(),
                        node.name.clone(),
                        format!("{:?}", node.node_type).to_lowercase(),
                        format!("{:?}", node.connection).to_lowercase(),
                        address(node),
                        format!("{:?}", node.status).to_lowercase(),
                    ]
                })
                .collect::<Vec<_>>();
            print_table(
                &["当前", "ID", "名称", "类型", "通道", "地址", "状态"],
                &rows,
            );
            Ok(())
        }
        NodeAction::Show { node } => {
            let state = config::load_state()?;
            let selector = node
                .or_else(|| state.current_node_id.clone())
                .ok_or_else(|| AppError::validation("尚未选择节点。"))?;
            let node = find_node(&state, &selector)?.clone();
            if json {
                return print_json(&node);
            }
            println!("ID：{}", node.id);
            println!("名称：{}", node.name);
            println!("备注：{}", node.alias);
            println!("类型：{:?}", node.node_type);
            println!("通道：{:?}", node.connection);
            println!("地址：{}", address(&node));
            println!("状态：{:?}", node.status);
            if let Some(username) = node.username.as_deref() {
                println!("SSH 用户：{username}");
            }
            if let Some(path) = node.private_key_path.as_deref() {
                println!("私钥路径：{path}");
            }
            if let Some(socket) = node.remote_socket.as_deref() {
                println!("远端 Socket：{socket}");
            }
            if let Some(time) = node.last_success_time {
                println!("最近连通：{}", format_timestamp(time));
            }
            Ok(())
        }
        NodeAction::Add(args) => {
            let input = build_node_input(&args, None)?;
            let prepared = config::prepare_node(input, None)?;
            if let Some((id, credential)) = &prepared.save_credential {
                save_credential_or_warn(id, credential)?;
            }
            let mut state = config::load_state()?;
            let node = prepared.node.clone();
            state.nodes.push(node.clone());
            config::write_state(&state)?;
            if json {
                print_json(&node)
            } else {
                println!("已新增节点 {}（{}）", node.name, node.id);
                Ok(())
            }
        }
        NodeAction::Edit { node, args } => {
            let mut state = config::load_state()?;
            let existing = find_node(&state, &node)?.clone();
            let input = build_node_input(&args, Some(&existing))?;
            let prepared = config::prepare_node(input, Some(&existing))?;
            if let Some((id, credential)) = &prepared.save_credential {
                save_credential_or_warn(id, credential)?;
            }
            let updated = prepared.node.clone();
            *config::find_node_mut(&mut state, &existing.id)? = updated.clone();
            config::write_state(&state)?;
            docker::disconnect_node(&existing.id);
            for credential_id in prepared.delete_credentials {
                credential::delete(&credential_id)?;
            }
            if json {
                print_json(&updated)
            } else {
                println!("已更新节点 {}（{}）", updated.name, updated.id);
                Ok(())
            }
        }
        NodeAction::Use { node } => {
            let mut state = config::load_state()?;
            let target = find_node(&state, &node)?.id.clone();
            state.current_node_id = Some(target.clone());
            config::write_state(&state)?;
            if json {
                print_json(&serde_json::json!({ "currentNodeId": target }))
            } else {
                println!("当前节点已切换为 {target}");
                Ok(())
            }
        }
        NodeAction::Test { node } => {
            let mut state = config::load_state()?;
            let selector = node
                .or_else(|| state.current_node_id.clone())
                .ok_or_else(|| AppError::validation("尚未选择节点。"))?;
            let id = find_node(&state, &selector)?.id.clone();
            let index = state
                .nodes
                .iter()
                .position(|node| node.id == id)
                .ok_or_else(|| AppError::validation("目标舰船不存在或已被移出舰队。"))?;
            docker::disconnect_node(&id);
            let result = docker::check_connection(&state.nodes[index]).await;
            match result {
                Ok(()) => {
                    state.nodes[index].status = NodeStatus::Online;
                    state.nodes[index].last_success_time = Some(timestamp_ms());
                    config::write_state(&state)?;
                    if json {
                        print_json(&state.nodes[index])
                    } else {
                        println!("连接成功：{}（{}）", state.nodes[index].name, id);
                        Ok(())
                    }
                }
                Err(error) => {
                    state.nodes[index].status = NodeStatus::Offline;
                    config::write_state(&state)?;
                    Err(error)
                }
            }
        }
        NodeAction::Rm { node } => {
            let mut state = config::load_state()?;
            let existing = find_node(&state, &node)?.clone();
            let original_len = state.nodes.len();
            state.nodes.retain(|node| node.id != existing.id);
            if state.nodes.len() == original_len {
                return Err(AppError::validation("目标舰船不存在或已被移出舰队。"));
            }
            if state.current_node_id.as_deref() == Some(existing.id.as_str()) {
                state.current_node_id = state.nodes.first().map(|node| node.id.clone());
            }
            if let Some(credential_id) = existing.credential_id {
                credential::delete(&credential_id)?;
            }
            config::write_state(&state)?;
            docker::disconnect_node(&existing.id);
            if json {
                print_json(&serde_json::json!({ "removed": existing.id }))
            } else {
                println!("已移除节点 {}（{}）", existing.name, existing.id);
                Ok(())
            }
        }
    }
}

async fn list_containers(node_selector: Option<&str>, json: bool) -> Result<(), AppError> {
    let node = active_node(node_selector)?;
    let containers = container::list_for_node(&node).await?;
    if json {
        return print_json(&containers);
    }
    let rows = containers
        .iter()
        .map(|container| {
            vec![
                container.short_id.clone(),
                container.name.clone(),
                container.image.clone(),
                container.state.clone(),
                container.status.clone(),
                format_ports(&container.ports),
                format!("{:.1}%", container.cpu_percent),
                human_size(container.memory_usage as i64),
            ]
        })
        .collect::<Vec<_>>();
    print_table(
        &[
            "集装箱",
            "名称",
            "图纸",
            "状态",
            "详情",
            "端口",
            "CPU",
            "内存",
        ],
        &rows,
    );
    Ok(())
}

async fn create_container(
    node_selector: Option<&str>,
    args: CreateArgs,
    json: bool,
) -> Result<(), AppError> {
    let node = active_node(node_selector)?;
    let input = container::CreateContainerInput {
        name: args.name,
        image: args.image,
        command: args.command.unwrap_or_default(),
        env: args.env,
        ports: args
            .ports
            .iter()
            .map(|port| parse_port(port))
            .collect::<Result<_, _>>()?,
        volumes: args
            .volumes
            .iter()
            .map(|volume| parse_volume(volume))
            .collect::<Result<_, _>>()?,
        restart_policy: args.restart,
        network: args.network.unwrap_or_default(),
        auto_remove: args.auto_remove,
        start_after_create: args.start,
    };
    let id = container::create_for_node(&node, input).await?;
    if json {
        print_json(&serde_json::json!({ "node": node.name, "containerId": id }))
    } else {
        println!("已创建集装箱 {id}");
        Ok(())
    }
}

async fn handle_image(
    action: ImageAction,
    node_selector: Option<&str>,
    json: bool,
) -> Result<(), AppError> {
    let node = active_node(node_selector)?;
    match action {
        ImageAction::List => {
            let images = image::list_for_node(&node).await?;
            if json {
                return print_json(&images);
            }
            let rows = images
                .iter()
                .map(|image| {
                    vec![
                        if image.repo_tags.is_empty() {
                            "<none>".to_string()
                        } else {
                            image.repo_tags.join(", ")
                        },
                        image.short_id.clone(),
                        human_size(image.size),
                        format_timestamp(image.created.max(0) as u64 * 1000),
                    ]
                })
                .collect::<Vec<_>>();
            print_table(&["图纸", "ID", "大小", "创建时间"], &rows);
            Ok(())
        }
        ImageAction::Pull {
            reference,
            registry,
        } => {
            let result = image::pull_for_node(&node, &reference, registry.as_deref()).await?;
            if json {
                print_json(&result)
            } else {
                println!("已获取 {}：", result.reference);
                println!("{}", result.output);
                Ok(())
            }
        }
        ImageAction::Rm { image, force } => {
            image::remove_for_node(&node, &image, force).await?;
            if json {
                print_json(&serde_json::json!({ "removed": image }))
            } else {
                println!("已移除图纸 {image}");
                Ok(())
            }
        }
        ImageAction::Search { term } => {
            let results = image::search_for_node(&node, &term).await?;
            if json {
                return print_json(&results);
            }
            let rows = results
                .iter()
                .map(|result| {
                    vec![
                        result.name.clone(),
                        result.star_count.to_string(),
                        if result.is_official {
                            "官方".to_string()
                        } else {
                            "-".to_string()
                        },
                        result.description.clone(),
                    ]
                })
                .collect::<Vec<_>>();
            print_table(&["图纸", "星标", "官方", "描述"], &rows);
            Ok(())
        }
        ImageAction::Export {
            reference,
            destination,
        } => {
            image::export_for_node(&node, &reference, &destination).await?;
            if json {
                print_json(
                    &serde_json::json!({ "exported": reference, "destination": destination }),
                )
            } else {
                println!("已导出 {reference} 到 {destination}");
                Ok(())
            }
        }
        ImageAction::Import { source } => {
            let result = image::import_for_node(&node, &source).await?;
            if json {
                print_json(&result)
            } else {
                println!("已导入 {}：", result.reference);
                println!("{}", result.output);
                Ok(())
            }
        }
    }
}

async fn handle_compose(
    action: ComposeAction,
    node_selector: Option<&str>,
    json: bool,
) -> Result<(), AppError> {
    match action {
        ComposeAction::List => {
            let mut projects = compose::load_projects()?;
            projects.sort_by(|left, right| right.updated_time.cmp(&left.updated_time));
            if json {
                return print_json(&projects);
            }
            let rows = projects
                .iter()
                .map(|project| {
                    vec![
                        project.id.chars().take(8).collect(),
                        project.name.clone(),
                        format_timestamp(project.updated_time),
                    ]
                })
                .collect::<Vec<_>>();
            print_table(&["ID", "名称", "更新时间"], &rows);
            Ok(())
        }
        ComposeAction::Ps => {
            let node = active_node(node_selector)?;
            let deployments = compose::list_for_node(&node).await?;
            if json {
                return print_json(&deployments);
            }
            let rows = deployments
                .iter()
                .map(|deployment| {
                    vec![
                        deployment.name.clone(),
                        format!("{}/{}", deployment.running, deployment.containers),
                        deployment.services.join(", "),
                        deployment.state.clone(),
                    ]
                })
                .collect::<Vec<_>>();
            print_table(&["编队", "运行/总数", "服务", "状态"], &rows);
            Ok(())
        }
        ComposeAction::Up { project } => {
            let node = active_node(node_selector)?;
            let project = compose::find_project(&project)?;
            let result = compose::up_for_node(&node, &project).await?;
            if json {
                print_json(&result)
            } else {
                println!("{}", result.output);
                Ok(())
            }
        }
        ComposeAction::Down { project } => {
            let node = active_node(node_selector)?;
            let project = compose::find_project(&project)?;
            let result = compose::down_for_node(&node, &project).await?;
            if json {
                print_json(&result)
            } else {
                println!("{}", result.output);
                Ok(())
            }
        }
        ComposeAction::Logs { project, tail } => {
            let node = active_node(node_selector)?;
            let project = compose::find_project(&project)?;
            let output = compose::logs_for_node(&node, &project.name, tail).await?;
            if json {
                print_json(&serde_json::json!({ "project": project.name, "logs": output }))
            } else {
                print!("{output}");
                Ok(())
            }
        }
        ComposeAction::Save { name, file, id } => {
            let yaml = std::fs::read_to_string(&file).map_err(|error| {
                AppError::input(format!("无法读取编队文件 {}：{error}", file.display()))
            })?;
            let project = compose::save_project(&name, &yaml, id.as_deref())?;
            if json {
                print_json(&project)
            } else {
                println!("已保存编队 {}（{}）", project.name, project.id);
                Ok(())
            }
        }
        ComposeAction::Rm { project } => {
            let target = compose::find_project(&project)?;
            compose::remove_project(&target.id)?;
            if json {
                print_json(&serde_json::json!({ "removed": target.id }))
            } else {
                println!("已移除编队 {}", target.name);
                Ok(())
            }
        }
        ComposeAction::Validate { file } => {
            let yaml = if file == "-" {
                let mut buffer = String::new();
                std::io::stdin()
                    .read_to_string(&mut buffer)
                    .map_err(|error| AppError::input(format!("读取标准输入失败：{error}")))?;
                buffer
            } else {
                std::fs::read_to_string(&file)
                    .map_err(|error| AppError::input(format!("无法读取编队文件 {file}：{error}")))?
            };
            let services = compose::validate_yaml(&yaml)?;
            if json {
                print_json(&serde_json::json!({ "services": services }))
            } else {
                println!(
                    "校验通过，共 {} 个服务：{}",
                    services.len(),
                    services.join(", ")
                );
                Ok(())
            }
        }
    }
}

fn handle_registry(action: RegistryAction, json: bool) -> Result<(), AppError> {
    match action {
        RegistryAction::List => print_registries(json),
        RegistryAction::Add { name, url, default } => {
            let (name, url) = config::normalize_registry(name, url)?;
            let mut registries = config::load_registries()?;
            let id = uuid::Uuid::new_v4().to_string();
            registries.push(RegistrySource {
                id: id.clone(),
                name,
                url,
                is_default: false,
            });
            config::ensure_default_registry(&mut registries, default.then_some(id.as_str()))?;
            config::write_registries(&registries)?;
            if json {
                print_json(&registries)
            } else {
                println!("已新增补给港 {id}");
                print_registries(false)
            }
        }
        RegistryAction::Use { registry } => {
            let mut registries = config::load_registries()?;
            let target = find_registry(&registries, &registry)?.id.clone();
            for entry in &mut registries {
                entry.is_default = entry.id == target;
            }
            config::write_registries(&registries)?;
            if json {
                print_json(&registries)
            } else {
                println!("默认补给港已切换为 {target}");
                Ok(())
            }
        }
        RegistryAction::Rm { registry } => {
            let mut registries = config::load_registries()?;
            let target = find_registry(&registries, &registry)?.id.clone();
            if target == "docker-hub" {
                return Err(AppError::validation("官方补给港不可移除。"));
            }
            registries.retain(|entry| entry.id != target);
            config::ensure_default_registry(&mut registries, None)?;
            config::write_registries(&registries)?;
            if json {
                print_json(&registries)
            } else {
                println!("已移除补给港 {target}");
                print_registries(false)
            }
        }
    }
}

fn print_registries(json: bool) -> Result<(), AppError> {
    let registries = config::load_registries()?;
    if json {
        return print_json(&registries);
    }
    let rows = registries
        .iter()
        .map(|registry| {
            vec![
                if registry.is_default {
                    "*".to_string()
                } else {
                    String::new()
                },
                registry.id.clone(),
                registry.name.clone(),
                registry.url.clone(),
            ]
        })
        .collect::<Vec<_>>();
    print_table(&["默认", "ID", "名称", "地址"], &rows);
    Ok(())
}

async fn monitor(json: bool, watch: bool, interval: u64) -> Result<(), AppError> {
    loop {
        let state = config::load_state()?;
        let results = futures_util::future::join_all(state.nodes.iter().map(|node| async move {
            let result = container::overview_for_node(node).await;
            (node.clone(), result)
        }))
        .await;
        if json {
            let entries = results
                .iter()
                .map(|(node, result)| {
                    let mut value = serde_json::json!({
                        "node": node.name,
                        "id": node.id,
                        "connection": format!("{:?}", node.connection).to_lowercase(),
                    });
                    match result {
                        Ok((containers, running, images)) => {
                            value["containers"] = (*containers).into();
                            value["running"] = (*running).into();
                            value["images"] = (*images).into();
                            value["online"] = true.into();
                        }
                        Err(error) => {
                            value["online"] = false.into();
                            value["error"] = error.message().into();
                        }
                    }
                    value
                })
                .collect::<Vec<_>>();
            print_json(&entries)?;
        } else {
            if watch {
                print!("\x1b[2J\x1b[H");
            }
            let rows = results
                .iter()
                .map(|(node, result)| match result {
                    Ok((containers, running, images)) => vec![
                        node.name.clone(),
                        format!("{:?}", node.connection).to_lowercase(),
                        containers.to_string(),
                        running.to_string(),
                        images.to_string(),
                        "在线".to_string(),
                    ],
                    Err(_) => vec![
                        node.name.clone(),
                        format!("{:?}", node.connection).to_lowercase(),
                        "-".to_string(),
                        "-".to_string(),
                        "-".to_string(),
                        "离线".to_string(),
                    ],
                })
                .collect::<Vec<_>>();
            print_table(&["节点", "通道", "集装箱", "运行中", "图纸", "状态"], &rows);
        }
        if !watch {
            break;
        }
        tokio::time::sleep(Duration::from_secs(interval.max(1))).await;
    }
    Ok(())
}

fn info(json: bool) -> Result<(), AppError> {
    let path = config::default_config_path()?;
    let directory = config::config_directory()?;
    let state = config::load_state()?;
    let current = state
        .current_node_id
        .as_deref()
        .and_then(|id| state.nodes.iter().find(|node| node.id == id))
        .map(|node| node.name.clone());
    if json {
        return print_json(&serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "configPath": path,
            "configDirectory": directory,
            "nodes": state.nodes.len(),
            "currentNode": current,
        }));
    }
    println!("ShipWiz CLI {}", env!("CARGO_PKG_VERSION"));
    println!("配置文件：{}", path.display());
    println!("配置目录：{}", directory.display());
    println!("节点数量：{}", state.nodes.len());
    println!("当前节点：{}", current.unwrap_or_else(|| "-".to_string()));
    Ok(())
}

fn active_node(selector: Option<&str>) -> Result<ShipNode, AppError> {
    let state = config::load_state()?;
    let selector = match selector {
        Some(selector) => selector.to_string(),
        None => state
            .current_node_id
            .clone()
            .ok_or_else(|| AppError::validation("尚未选择节点，请先执行 node use。"))?,
    };
    find_node(&state, &selector).cloned()
}

fn find_node<'a>(state: &'a FleetState, selector: &str) -> Result<&'a ShipNode, AppError> {
    state
        .nodes
        .iter()
        .find(|node| node.id == selector || node.name == selector)
        .or_else(|| {
            state
                .nodes
                .iter()
                .find(|node| node.name.eq_ignore_ascii_case(selector))
        })
        .ok_or_else(|| {
            AppError::validation(format!(
                "找不到节点“{selector}”，请执行 node list 查看可用节点。"
            ))
        })
}

fn find_registry<'a>(
    registries: &'a [RegistrySource],
    selector: &str,
) -> Result<&'a RegistrySource, AppError> {
    registries
        .iter()
        .find(|registry| registry.id == selector || registry.name == selector)
        .ok_or_else(|| {
            AppError::validation(format!(
                "找不到补给港“{selector}”，请执行 registry list 查看。"
            ))
        })
}

fn build_node_input(args: &NodeArgs, existing: Option<&ShipNode>) -> Result<NodeInput, AppError> {
    let name = args
        .name
        .clone()
        .or_else(|| existing.map(|node| node.name.clone()))
        .ok_or_else(|| AppError::input("请通过 --name 指定舰船名称。"))?;
    let alias = args
        .alias
        .clone()
        .or_else(|| existing.map(|node| node.alias.clone()))
        .unwrap_or_default();
    let node_type = args
        .node_type
        .map(NodeType::from)
        .or_else(|| existing.map(|node| node.node_type))
        .unwrap_or(NodeType::Local);
    let connection = args
        .connection
        .map(NodeConnection::from)
        .or_else(|| existing.map(|node| node.connection))
        .unwrap_or(if node_type == NodeType::Local {
            NodeConnection::Local
        } else {
            NodeConnection::Tls
        });
    let host = args
        .host
        .clone()
        .or_else(|| existing.map(|node| node.host.clone()))
        .unwrap_or_else(|| "localhost".to_string());
    let port = args
        .port
        .or_else(|| existing.map(|node| node.port))
        .unwrap_or(match connection {
            NodeConnection::Tls => 2376,
            NodeConnection::Ssh => 22,
            NodeConnection::Local => 0,
        });
    let cert_path = args
        .cert_path
        .clone()
        .or_else(|| existing.and_then(|node| node.cert_path.clone()));
    let username = args
        .username
        .clone()
        .or_else(|| existing.and_then(|node| node.username.clone()));
    let auth_method = args
        .auth
        .map(SshAuthMethod::from)
        .or_else(|| existing.and_then(|node| node.auth_method))
        .or_else(|| (connection == NodeConnection::Ssh).then_some(SshAuthMethod::Password));
    let private_key_path = args
        .private_key_path
        .clone()
        .or_else(|| existing.and_then(|node| node.private_key_path.clone()));
    let remote_socket = args
        .remote_socket
        .clone()
        .or_else(|| existing.and_then(|node| node.remote_socket.clone()));
    let password = args
        .password
        .clone()
        .or_else(|| env_secret("SHIPWIZ_SSH_PASSWORD"));
    let private_key = args
        .private_key
        .clone()
        .or_else(|| env_secret("SHIPWIZ_SSH_PRIVATE_KEY"));
    let passphrase = args
        .passphrase
        .clone()
        .or_else(|| env_secret("SHIPWIZ_SSH_PASSPHRASE"));
    Ok(NodeInput {
        name,
        alias,
        node_type,
        connection,
        host,
        port,
        tls: connection == NodeConnection::Tls,
        cert_path,
        username,
        auth_method,
        private_key_path,
        remote_socket,
        password,
        private_key,
        passphrase,
        clear_credential: false,
    })
}

fn save_credential_or_warn(id: &str, credential: &StoredCredential) -> Result<(), AppError> {
    match credential::save(id, credential) {
        Ok(()) => Ok(()),
        Err(error) => {
            if env_secret("SHIPWIZ_SSH_PASSWORD").is_some()
                || env_secret("SHIPWIZ_SSH_PRIVATE_KEY").is_some()
            {
                eprintln!(
                    "警告：无法写入系统密钥库，连接时将使用环境变量中的凭据。{}",
                    error.message()
                );
                Ok(())
            } else {
                Err(error)
            }
        }
    }
}

fn parse_port(value: &str) -> Result<container::PortInput, AppError> {
    let (mapping, protocol) = match value.split_once('/') {
        Some((mapping, protocol)) => (mapping, protocol.to_string()),
        None => (value, "tcp".to_string()),
    };
    let (host_port, container_port) = mapping
        .split_once(':')
        .ok_or_else(|| AppError::input(format!("端口映射“{value}”应为 host:container。")))?;
    Ok(container::PortInput {
        host_port: host_port
            .parse()
            .map_err(|_| AppError::input(format!("主机端口“{host_port}”无效。")))?,
        container_port: container_port
            .parse()
            .map_err(|_| AppError::input(format!("集装箱端口“{container_port}”无效。")))?,
        protocol,
        host_ip: String::new(),
    })
}

fn parse_volume(value: &str) -> Result<container::VolumeInput, AppError> {
    let mut parts = value.split(':');
    let source = parts
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| AppError::input(format!("挂载“{value}”缺少来源路径。")))?;
    let target = parts
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| AppError::input(format!("挂载“{value}”缺少目标路径。")))?;
    let mode = parts.next().unwrap_or("rw");
    if parts.next().is_some() {
        return Err(AppError::input(format!(
            "挂载“{value}”格式应为 source:target[:ro]。"
        )));
    }
    Ok(container::VolumeInput {
        source: source.to_string(),
        target: target.to_string(),
        read_only: mode == "ro",
    })
}

fn action_done(json: bool, action: &str, node: &ShipNode, container: &str) -> Result<(), AppError> {
    if json {
        print_json(&serde_json::json!({
            "action": action,
            "node": node.name,
            "container": container,
            "ok": true,
        }))
    } else {
        println!("已对集装箱 {container} 执行 {action}（节点 {}）", node.name);
        Ok(())
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<(), AppError> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| AppError::input(format!("序列化输出失败：{error}")))?;
    println!("{text}");
    Ok(())
}

fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    let mut widths = headers
        .iter()
        .map(|header| header.chars().count())
        .collect::<Vec<_>>();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            if let Some(width) = widths.get_mut(index) {
                *width = (*width).max(cell.chars().count());
            }
        }
    }
    let render = |cells: &[String]| {
        cells
            .iter()
            .enumerate()
            .map(|(index, cell)| {
                let width = widths.get(index).copied().unwrap_or_default();
                format!("{cell:<width$}")
            })
            .collect::<Vec<_>>()
            .join("  ")
    };
    let header_cells = headers
        .iter()
        .map(|header| header.to_string())
        .collect::<Vec<_>>();
    println!("{}", render(&header_cells));
    println!(
        "{}",
        widths
            .iter()
            .map(|width| "-".repeat(*width))
            .collect::<Vec<_>>()
            .join("  ")
    );
    for row in rows {
        println!("{}", render(row));
    }
}

fn address(node: &ShipNode) -> String {
    match node.connection {
        NodeConnection::Local => "本机 Docker Socket".to_string(),
        NodeConnection::Tls => format!("tcp://{}:{}", node.host, node.port),
        NodeConnection::Ssh => format!(
            "ssh://{}@{}:{}",
            node.username.as_deref().unwrap_or("-"),
            node.host,
            node.port
        ),
    }
}

fn format_ports(ports: &[container::ContainerPort]) -> String {
    if ports.is_empty() {
        return "-".to_string();
    }
    ports
        .iter()
        .map(|port| match port.public_port {
            Some(public) => format!("{public}->{}/{}", port.private_port, port.protocol),
            None => format!("{}/{}", port.private_port, port.protocol),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn human_size(bytes: i64) -> String {
    let mut value = bytes.max(0) as f64;
    let mut unit = "B";
    for next in ["KB", "MB", "GB", "TB"] {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = next;
    }
    if unit == "B" {
        format!("{value:.0}{unit}")
    } else {
        format!("{value:.1}{unit}")
    }
}

fn format_timestamp(ms: u64) -> String {
    let seconds = (ms / 1000) as i64;
    let days = seconds.div_euclid(86_400);
    let remaining = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        remaining / 3600,
        (remaining % 3600) / 60,
        remaining % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (year, month as u32, day as u32)
}

fn env_secret(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}
