# shipwiz-cli

ShipWiz 终端 Docker 舰队管理器（TUI）。安装后从 GitHub Release 下载对应平台的独立二进制，不依赖 Node 运行时。

```bash
npm install -g shipwiz-cli
shipwiz-cli
```

启动后进入全屏界面：`1`–`6` 切换集装箱 / 图纸 / 编队 / 节点 / 补给港 / 帮助，`F5` 刷新，`?` 查看按键手册，`q` 退出。

支持 Linux x64、Windows x64、macOS arm64/x64。若下载失败，可从
[GitHub Releases](https://github.com/Lensi-zhang/ShipWiz/releases) 手动获取二进制。

设置 `SHIPWIZ_CLI_SKIP_DOWNLOAD=1` 可跳过 postinstall 下载（用于离线打包）。
