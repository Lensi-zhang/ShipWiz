#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawn } = require("node:child_process");

const binary = path.join(
  __dirname,
  process.platform === "win32" ? "shipwiz-cli.exe" : "shipwiz-cli"
);

if (!fs.existsSync(binary)) {
  console.error(
    "未找到 ShipWiz CLI 二进制，请重新执行 npm install shipwiz-cli，或设置 SHIPWIZ_CLI_SKIP_DOWNLOAD=0 后重试。"
  );
  process.exit(1);
}

const child = spawn(binary, process.argv.slice(2), { stdio: "inherit" });
child.on("error", (error) => {
  console.error(`启动 ShipWiz CLI 失败：${error.message}`);
  process.exit(1);
});
child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code === null ? 1 : code);
});
