#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const https = require("node:https");
const path = require("node:path");

const VERSION = require("./package.json").version;
const REPO = "Lensi-zhang/ShipWiz";
const MAX_REDIRECTS = 5;
const MAX_ATTEMPTS = 6;
const IDLE_TIMEOUT = 120_000;

function resolveAsset() {
  const platform = process.platform;
  const arch = process.arch;
  if (platform === "linux" && arch === "x64") {
    return { asset: "shipwiz-cli-linux-x64", binary: "shipwiz-cli" };
  }
  if (platform === "win32" && arch === "x64") {
    return { asset: "shipwiz-cli-windows-x64.exe", binary: "shipwiz-cli.exe" };
  }
  if (platform === "darwin" && arch === "arm64") {
    return { asset: "shipwiz-cli-darwin-arm64", binary: "shipwiz-cli" };
  }
  if (platform === "darwin" && arch === "x64") {
    return { asset: "shipwiz-cli-darwin-x64", binary: "shipwiz-cli" };
  }
  return null;
}

function fetchTo(url, destination, redirects = 0) {
  return new Promise((resolve, reject) => {
    const temp = `${destination}.download`;
    const downloaded = fs.existsSync(temp) ? fs.statSync(temp).size : 0;
    const headers = { "User-Agent": "shipwiz-cli-npm" };
    if (downloaded > 0) {
      headers.Range = `bytes=${downloaded}-`;
    }
    const request = https.get(url, { headers }, (response) => {
      const status = response.statusCode || 0;
      if (status >= 300 && status < 400 && response.headers.location) {
        response.resume();
        if (redirects >= MAX_REDIRECTS) {
          reject(new Error("重定向次数过多"));
          return;
        }
        const next = new URL(response.headers.location, url).toString();
        resolve(fetchTo(next, destination, redirects + 1));
        return;
      }
      if (status !== 200 && status !== 206) {
        response.resume();
        reject(new Error(`下载失败，HTTP ${status}`));
        return;
      }
      const append = status === 206 && downloaded > 0;
      const file = fs.createWriteStream(temp, { flags: append ? "a" : "w" });
      response.on("error", (error) => {
        file.destroy();
        reject(error);
      });
      file.on("error", reject);
      file.on("finish", () => file.close(() => resolve()));
      response.pipe(file);
    });
    request.on("error", reject);
    request.setTimeout(IDLE_TIMEOUT, () => {
      request.destroy(new Error("下载超时"));
    });
  });
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function download(url, destination) {
  const temp = `${destination}.download`;
  let lastError = new Error("下载失败");
  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt += 1) {
    try {
      await fetchTo(url, destination);
      const size = fs.existsSync(temp) ? fs.statSync(temp).size : 0;
      if (size === 0) {
        throw new Error("下载内容为空");
      }
      fs.renameSync(temp, destination);
      return;
    } catch (error) {
      lastError = error;
      if (attempt < MAX_ATTEMPTS) {
        console.error(`第 ${attempt} 次下载未完成（${error.message}），正在续传…`);
        await delay(attempt * 2_000);
      }
    }
  }
  fs.rmSync(temp, { force: true });
  throw lastError;
}

async function main() {
  const target = resolveAsset();
  if (!target) {
    console.error(
      `ShipWiz CLI 暂不支持当前平台 ${process.platform}/${process.arch}。`
    );
    console.error(
      `可从 https://github.com/${REPO}/releases 手动下载，或从源码执行 cargo build --no-default-features --bin shipwiz-cli。`
    );
    process.exit(1);
  }
  if (process.env.SHIPWIZ_CLI_SKIP_DOWNLOAD === "1") {
    console.log("已跳过 ShipWiz CLI 二进制下载。");
    return;
  }

  const binDirectory = path.join(__dirname, "bin");
  fs.mkdirSync(binDirectory, { recursive: true });
  const destination = path.join(binDirectory, target.binary);
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${target.asset}`;

  console.log(`正在下载 ShipWiz CLI v${VERSION}（${target.asset}）...`);
  try {
    await download(url, destination);
    if (process.platform !== "win32") {
      fs.chmodSync(destination, 0o755);
    }
    console.log(`ShipWiz CLI 已安装：${destination}`);
  } catch (error) {
    console.error(`下载 ShipWiz CLI 失败：${error.message}`);
    console.error(`请手动从 ${url} 下载并保存为 ${destination}。`);
    process.exit(1);
  }
}

main();
