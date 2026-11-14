use regex::Regex;
#[cfg(feature = "desktop")]
use serde::Serialize;
use std::fmt::Display;

#[derive(Debug, Clone)]
pub struct FriendlyError {
    title: &'static str,
    reason: String,
    solution: &'static str,
}

impl FriendlyError {
    pub fn message(&self) -> String {
        format!(
            "{}\n原因：{}\n解决方案：{}",
            self.title, self.reason, self.solution
        )
    }
}

#[derive(Debug, Clone)]
pub struct AppError {
    friendly: FriendlyError,
}

impl AppError {
    pub fn validation(reason: impl Into<String>) -> Self {
        Self::from_friendly(FriendlyError {
            title: "舰船配置需要调整",
            reason: reason.into(),
            solution: "请修改配置后重新保存。",
        })
    }

    pub fn input(reason: impl Into<String>) -> Self {
        Self::from_friendly(FriendlyError {
            title: "装载参数需要调整",
            reason: reason.into(),
            solution: "请修改集装箱配置后重新提交。",
        })
    }

    pub fn compose(reason: impl Into<String>) -> Self {
        Self::from_friendly(FriendlyError {
            title: "编队引擎尚未就绪",
            reason: reason.into(),
            solution:
                "请安装或修复 Docker Compose 插件，并确认 docker compose version 可正常执行。",
        })
    }

    pub fn config(error: impl Display) -> Self {
        Self::from_friendly(FriendlyError {
            title: "航海配置暂时无法读写",
            reason: error.to_string(),
            solution: "请检查 ~/.shipwiz/config.json 的目录权限，或备份后移除损坏的配置文件。",
        })
    }

    pub fn docker(error: impl Display) -> Self {
        let raw = error.to_string();
        Self::from_friendly(translate_docker_error(&raw))
    }

    pub fn ssh(error: impl Display) -> Self {
        Self::from_friendly(FriendlyError {
            title: "SSH 隧道暂时无法连接",
            reason: error.to_string(),
            solution: "请检查 SSH 地址、认证凭据和远端 Docker Socket，然后手动重试。",
        })
    }

    pub fn host_key(error: impl Display) -> Self {
        Self::from_friendly(FriendlyError {
            title: "SSH 主机身份校验未通过",
            reason: error.to_string(),
            solution: "如果目标舰船确实重装或更换了主机密钥，请删除系统配置目录下 known_hosts.json 中对应记录后重新连接；否则请立即停止连接并排查网络中间人风险。",
        })
    }

    pub fn credential(error: impl Display) -> Self {
        Self::from_friendly(FriendlyError {
            title: "节点凭据暂时无法使用",
            reason: error.to_string(),
            solution: "请确认系统密钥库可用，并重新输入节点凭据。",
        })
    }

    pub fn terminal(error: impl Display) -> Self {
        Self::from_friendly(FriendlyError {
            title: "集装箱终端暂时不可用",
            reason: error.to_string(),
            solution: "请确认集装箱正在运行、终端会话仍然有效，然后重试。",
        })
    }

    fn from_friendly(friendly: FriendlyError) -> Self {
        Self { friendly }
    }

    pub fn message(&self) -> String {
        self.friendly.message()
    }
}

fn translate_docker_error(raw: &str) -> FriendlyError {
    let rules = [
        (
            r"(?i)address already in use|port is already allocated|port.*already.*use",
            "航道端口被占用",
            "目标端口已经被其他服务使用。",
            "请更换未占用的端口，或先停止占用该端口的服务。",
        ),
        (
            r"(?i)timed out|timeout|deadline has elapsed",
            "港口响应缓慢",
            "连接在规定时间内没有收到响应。",
            "请检查网络与舰船状态；采购图纸时可尝试切换补给港。",
        ),
        (
            r"(?i)authentication required|unauthorized.*(registry|repository|pull)|pull access denied",
            "补给港拒绝了采购请求",
            "目标图纸可能需要授权，或当前账户没有采购权限。",
            "请核对图纸名称与仓库权限，并先在动力引擎中完成对应港口登录。",
        ),
        (
            r"(?i)permission denied|access is denied|unauthorized",
            "动力引擎访问受限",
            "当前账户没有足够的连接权限。",
            "请确认当前账户已加入引擎管理组，并以适当权限重新启动 ShipWiz。",
        ),
        (
            r"(?i)certificate|x509|tls|ssl",
            "安全证书校验失败",
            "远程舰船的 TLS 证书缺失、无效或不受信任。",
            "请确认证书目录包含 ca.pem、cert.pem 和 key.pem，并检查证书是否匹配目标舰船。",
        ),
        (
            r"(?i)image.*being used|image.*referenced|image.*has dependent",
            "货箱图纸仍在使用中",
            "已有集装箱引用该图纸，或存在其他关联图纸。",
            "请先撤收相关集装箱；确认无引用后，再移除图纸。",
        ),
        (
            r"(?i)no such image|image.*not found|manifest unknown|repository.*not found",
            "没有找到货箱图纸",
            "指定图纸不存在或尚未采购到本舰。",
            "请核对图纸名称与标签，必要时先执行采购。",
        ),
        (
            r"(?i)no such container|container.*not found",
            "没有找到目标集装箱",
            "集装箱可能已经被撤收，或当前舰船已经切换。",
            "请刷新集装箱列表，确认目标仍然存在后重试。",
        ),
        (
            r"(?i)container.*already.*running|container.*not running",
            "集装箱状态已经变化",
            "当前状态不支持刚才执行的生命周期操作。",
            "请刷新列表获取最新状态，再选择可用操作。",
        ),
        (
            r"(?i)conflict.*container name|name.*already in use",
            "集装箱名称已被使用",
            "同一艘舰船上不能存在两个同名集装箱。",
            "请更换名称，或先撤收使用同名集装箱的旧实例。",
        ),
        (
            r"(?i)cannot connect|connection refused|error trying to connect|no such file|cannot find the file|os error 10061|npipe",
            "无法连接舰船动力引擎",
            "动力引擎可能尚未启动，或系统连接通道不可用。",
            "请先启动本机动力引擎；若为远程舰船，请检查地址、端口与 TLS 服务配置。",
        ),
    ];

    for (pattern, title, reason, solution) in rules {
        if Regex::new(pattern)
            .map(|rule| rule.is_match(raw))
            .unwrap_or(false)
        {
            return FriendlyError {
                title,
                reason: format!("{reason}\n诊断信息：{raw}"),
                solution,
            };
        }
    }

    FriendlyError {
        title: "航行操作未完成",
        reason: format!("动力引擎返回了未分类错误。\n诊断信息：{raw}"),
        solution: "请根据诊断信息检查舰船配置，然后重试。",
    }
}

#[cfg(feature = "desktop")]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    pub code: u16,
    pub msg: String,
    pub data: Option<T>,
}

#[cfg(feature = "desktop")]
impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            code: 0,
            msg: "操作成功".to_string(),
            data: Some(data),
        }
    }

    pub fn failure(error: AppError) -> Self {
        Self {
            code: 1,
            msg: error.message(),
            data: None,
        }
    }
}

#[cfg(feature = "desktop")]
pub fn respond<T>(result: Result<T, AppError>) -> ApiResponse<T> {
    match result {
        Ok(data) => ApiResponse::success(data),
        Err(error) => ApiResponse::failure(error),
    }
}
