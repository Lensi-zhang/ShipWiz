#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

export const expectedPlatforms = [
  "darwin-aarch64",
  "darwin-aarch64-app",
  "darwin-x86_64",
  "darwin-x86_64-app",
  "linux-x86_64",
  "linux-x86_64-appimage",
  "linux-x86_64-deb",
  "windows-x86_64",
  "windows-x86_64-nsis",
  "windows-x86_64-msi",
];

const usage =
  "Usage: node scripts/normalize-updater.mjs <manifest.json> <release.json> <output.json> <expected-version>";

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function requireNonEmptyString(value, description) {
  if (typeof value !== "string" || !value.trim()) {
    throw new Error(`${description} must be a non-empty string`);
  }
  return value;
}

export function normalizeUpdaterManifest(
  manifest,
  releaseAssets,
  expectedVersion,
) {
  if (!isObject(manifest) || !isObject(manifest.platforms)) {
    throw new Error("Updater manifest must contain a platforms object");
  }
  if (!Array.isArray(releaseAssets)) {
    throw new Error("Release assets must be an array");
  }
  requireNonEmptyString(expectedVersion, "Expected version");
  if (manifest.version !== expectedVersion) {
    throw new Error(
      `Updater version ${JSON.stringify(manifest.version)} does not match ${JSON.stringify(expectedVersion)}`,
    );
  }

  const actualPlatforms = Object.keys(manifest.platforms);
  const expected = new Set(expectedPlatforms);
  const actual = new Set(actualPlatforms);
  const missing = expectedPlatforms.filter((platform) => !actual.has(platform));
  const extra = actualPlatforms.filter((platform) => !expected.has(platform));
  if (
    missing.length ||
    extra.length ||
    actual.size !== actualPlatforms.length
  ) {
    throw new Error(
      `Updater platform mismatch; missing: ${missing.join(", ") || "none"}; unexpected: ${extra.join(", ") || "none"}`,
    );
  }

  const downloadUrls = new Map();
  for (const [index, asset] of releaseAssets.entries()) {
    if (!isObject(asset)) {
      throw new Error(`Release asset ${index} must be an object`);
    }
    const apiUrl = requireNonEmptyString(
      asset.url,
      `Release asset ${index} API URL`,
    );
    const publicUrl = requireNonEmptyString(
      asset.browser_download_url,
      `Release asset ${index} public URL`,
    );
    for (const url of [apiUrl, publicUrl]) {
      const existing = downloadUrls.get(url);
      if (existing !== undefined && existing !== publicUrl) {
        throw new Error(`Duplicate asset mapping for ${url}`);
      }
      downloadUrls.set(url, publicUrl);
    }
  }

  const normalized = structuredClone(manifest);
  for (const platform of expectedPlatforms) {
    const entry = normalized.platforms[platform];
    if (!isObject(entry)) {
      throw new Error(`Updater platform ${platform} must be an object`);
    }
    requireNonEmptyString(
      entry.signature,
      `Updater platform ${platform} signature`,
    );
    const sourceUrl = requireNonEmptyString(
      entry.url,
      `Updater platform ${platform} URL`,
    );
    const publicUrl = downloadUrls.get(sourceUrl);
    if (!publicUrl) {
      throw new Error(`Unknown updater asset for ${platform}: ${sourceUrl}`);
    }
    entry.url = publicUrl;
  }

  return normalized;
}

async function main() {
  const [manifestPath, releasePath, outputPath, expectedVersion, ...extra] =
    process.argv.slice(2);
  if (
    !manifestPath ||
    !releasePath ||
    !outputPath ||
    !expectedVersion ||
    extra.length
  ) {
    throw new Error(usage);
  }

  const [manifestText, releaseText] = await Promise.all([
    readFile(manifestPath, "utf8"),
    readFile(releasePath, "utf8"),
  ]);
  const manifest = JSON.parse(manifestText);
  const release = JSON.parse(releaseText);
  const normalized = normalizeUpdaterManifest(
    manifest,
    release.assets,
    expectedVersion,
  );
  await writeFile(
    outputPath,
    `${JSON.stringify(normalized, null, 2)}\n`,
    "utf8",
  );
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  try {
    await main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    console.error(usage);
    process.exitCode = 1;
  }
}
