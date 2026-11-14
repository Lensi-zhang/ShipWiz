# shipwiz-cli

ShipWiz 命令行 Docker 舰队管理器。安装后从 GitHub Release 下载对应平台的独立二进制，不依赖 Node 运行时。

```bash
npm install -g shipwiz-cli
shipwiz-cli --help
shipwiz-cli node list
shipwiz-cli ps
```

支持 Linux x64、Windows x64、macOS arm64/x64。若下载失败，可从
[GitHub Releases](https://github.com/Lensi-zhang/ShipWiz/releases) 手动获取二进制。

设置 `SHIPWIZ_CLI_SKIP_DOWNLOAD=1` 可跳过 postinstall 下载（用于离线打包）。
