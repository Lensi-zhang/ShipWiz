# ShipWiz

轻量级 Docker 桌面客户端，以航海为界面主题。基于 Tauri 2 + Vue 3 + Rust，可通过本机、TLS 或 SSH 三种通道连接 Docker 引擎。

## 功能

- **舰队（节点）**：本机 / TLS / SSH 三种接入方式，连接检测与在线状态
- **集装箱（容器）**：卡片/表格视图、创建、启停、重启、删除、日志查看、容器内交互终端
- **图纸（镜像）**：搜索、拉取、删除、导出与导入
- **编队（Compose）**：YAML 编辑与校验、up/down、日志、模板
- **监控**：容器与引擎资源总览
- **补给港**：镜像仓库（Registry）管理与默认仓库设置
- **界面**：深海/浅海主题、200px/56px 折叠侧栏、加载/空态/错误态三态反馈

## 技术栈

| 层 | 选型 |
|---|---|
| 桌面框架 | Tauri 2 |
| 前端 | Vue 3、TypeScript、NaiveUI、Pinia、Vue Router、Vite |
| 终端与编辑器 | xterm.js、Monaco Editor |
| 后端 | Rust、Bollard（Docker API）、ssh2、keyring |
| 打包 | Vite + Tauri CLI |

## 目录结构

```
src-frontend/       Vue 3 前端（视图、组件、store、API 封装）
src-backend/        Rust 后端（Tauri 命令、Docker、SSH、终端、配置）
src-backend/src/cli CLI 入口（与桌面端共用配置和 Docker/SSH 逻辑）
npm/                shipwiz-cli 的 npm 安装包（从 GitHub Release 下载二进制）
capabilities/       Tauri 权限配置
tauri.conf.json     应用与窗口配置
Cargo.toml          Rust 依赖与构建入口
package.json        前端依赖与脚本
```

## 环境要求

- Node.js 18+ 与 npm
- Rust 1.77.2+（含 MSVC 工具链）
- Docker Engine / Docker Desktop（本机连接时需要）
- Windows 构建额外需要 Strawberry Perl 与 NASM：`ssh2` 的 vendored OpenSSL 依赖它们
- Linux 构建桌面端需要 `libwebkit2gtk-4.1-dev`、`libayatana-appindicator3-dev`、`librsvg2-dev`、`patchelf`、`libssl-dev`；只构建 CLI 则无需这些图形库

## 开发与构建

```bash
npm install          # 安装前端依赖
npm run tauri:dev    # 启动桌面开发模式
npm run build        # 前端类型检查 + 产物构建
npm run tauri:build  # 打包桌面应用
```

后端检查在仓库根目录执行：

```bash
cargo fmt --all -- --check
cargo check
cargo test --lib
```

仅构建命令行版本（不编译 Tauri，Linux 服务器无需安装 WebKit/GTK）：

```bash
cargo build --release --no-default-features --bin shipwiz-cli
./target/release/shipwiz-cli --help
```

## Linux 安装

桌面端在 Linux 上提供 `.deb` 与 `.AppImage`，由 GitHub Actions 在打 tag 时自动构建：

```bash
sudo dpkg -i shipwiz_0.1.0_amd64.deb   # Debian / Ubuntu
chmod +x ShipWiz_0.1.0_amd64.AppImage   # 其他发行版
./ShipWiz_0.1.0_amd64.AppImage
```

CLI 可直接下载独立二进制，或用 npm 安装：

```bash
npm install -g shipwiz-cli
shipwiz-cli info
```

## 命令行（shipwiz-cli）

CLI 与桌面端共用 `~/.shipwiz/config.json`、`known_hosts.json` 与系统密钥串，可在服务器上无界面运行。

```bash
shipwiz-cli node list                     # 列出节点（* 为当前节点）
shipwiz-cli node add --name prod --type remote --connection ssh \
  --host 10.0.0.8 --username root --auth key --private-key-path ~/.ssh/id_ed25519
shipwiz-cli node use prod                 # 切换当前节点
shipwiz-cli node test                     # 连通性检查
shipwiz-cli ps                            # 集装箱列表（含 CPU/内存）
shipwiz-cli logs web -f                   # 跟踪日志
shipwiz-cli exec web -- ls -al /app       # 容器内执行命令
shipwiz-cli image list / pull nginx:alpine / rmi <image>
shipwiz-cli compose list / ps / up <编队> / down <编队> / logs <编队>
shipwiz-cli registry list / add --name mirror --url https://registry.example.com
shipwiz-cli monitor --watch               # 多节点概览
```

所有读取类命令都支持 `--json`，全局 `-n/--node` 可临时指定节点。

无头服务器无法访问系统密钥串时，可用环境变量提供 SSH 凭据：

```bash
export SHIPWIZ_SSH_PASSWORD='...'          # 密码认证
export SHIPWIZ_SSH_PRIVATE_KEY="$(cat ~/.ssh/id_ed25519)"   # 私钥内容认证
export SHIPWIZ_SSH_PASSPHRASE='...'        # 私钥口令（可选）
```

## 发布

推送 `v*` tag 会触发 `.github/workflows/release.yml`：构建 Linux deb/AppImage，以及 Linux、Windows、macOS 的 `shipwiz-cli` 二进制并上传到 Release。npm 包位于 `npm/`，发布前先更新版本号：

```bash
npm publish npm
```

## 连接方式

- **本机**：使用 Docker 默认通道（Windows 为 Named Pipe，macOS/Linux 为 Unix Socket）
- **TLS**：证书目录需包含 `ca.pem`、`cert.pem`、`key.pem`；未加密的远程 TCP 连接会被拒绝
- **SSH**：支持密码或私钥认证（私钥内容或本机文件路径），通过 `direct-streamlocal` 转发远端 `/var/run/docker.sock`

## 安全性

- 密码、私钥内容与口令只写入系统密钥串，节点配置与日志中不包含秘密
- SSH 首次连接会记录服务端主机公钥到应用配置目录的 `known_hosts.json`；后续指纹不一致将立即终止连接，避免中间人攻击
- 服务器重装或主动轮换主机密钥后，删除 `known_hosts.json` 中对应条目即可重新建立信任

## 已知限制

- 不提供宿主机 Shell、SFTP、跳板机与 SSH agent 转发
- 节点在线状态依赖手动检测，没有后台状态推送
- 尚未在真实外部 SSH 主机上完成端到端验收
