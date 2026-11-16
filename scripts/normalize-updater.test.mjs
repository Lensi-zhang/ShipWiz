import assert from "node:assert/strict";
import test from "node:test";

import {
  expectedPlatforms,
  normalizeUpdaterManifest,
} from "./normalize-updater.mjs";

function fixtures() {
  const assets = [
    ["darwin-aarch64", "1001", "ShipWiz_aarch64.app.tar.gz"],
    ["darwin-x86_64", "1002", "ShipWiz_x64.app.tar.gz"],
    ["linux-x86_64-appimage", "1003", "ShipWiz_amd64.AppImage"],
    ["linux-x86_64-deb", "1004", "ShipWiz_amd64.deb"],
    ["windows-x86_64-nsis", "1005", "ShipWiz_x64-setup.exe"],
    ["windows-x86_64-msi", "1006", "ShipWiz_x64_en-US.msi"],
  ].map(([, id, name]) => ({
    url: `https://api.github.com/repos/Lensi-zhang/ShipWiz/releases/assets/${id}`,
    browser_download_url: `https://github.com/Lensi-zhang/ShipWiz/releases/download/v0.2.6/${name}`,
  }));
  const platformAssets = {
    "darwin-aarch64": assets[0],
    "darwin-aarch64-app": assets[0],
    "darwin-x86_64": assets[1],
    "darwin-x86_64-app": assets[1],
    "linux-x86_64": assets[2],
    "linux-x86_64-appimage": assets[2],
    "linux-x86_64-deb": assets[3],
    "windows-x86_64": assets[4],
    "windows-x86_64-nsis": assets[4],
    "windows-x86_64-msi": assets[5],
  };
  const manifest = {
    version: "0.2.6",
    notes: "Release notes",
    pub_date: "2026-11-16T00:00:00.000Z",
    platforms: Object.fromEntries(
      expectedPlatforms.map((platform) => [
        platform,
        {
          signature: `signature-${platform}`,
          url: platformAssets[platform].url,
        },
      ]),
    ),
  };
  manifest.platforms["linux-x86_64-deb"].url =
    platformAssets["linux-x86_64-deb"].browser_download_url;
  return { assets, manifest, platformAssets };
}

test("normalizes mixed URLs idempotently without changing other fields", () => {
  const { assets, manifest, platformAssets } = fixtures();
  const original = structuredClone(manifest);
  const expected = structuredClone(manifest);
  for (const platform of expectedPlatforms) {
    expected.platforms[platform].url =
      platformAssets[platform].browser_download_url;
  }

  const normalized = normalizeUpdaterManifest(
    manifest,
    assets,
    "0.2.6",
    "v0.2.6",
  );

  assert.deepEqual(normalized, expected);
  assert.deepEqual(manifest, original);
  assert.notEqual(normalized, manifest);
  assert.deepEqual(
    normalizeUpdaterManifest(normalized, assets, "0.2.6", "v0.2.6"),
    expected,
  );
  for (const platform of expectedPlatforms) {
    assert.equal(
      normalized.platforms[platform].signature,
      original.platforms[platform].signature,
    );
    assert.ok(
      normalized.platforms[platform].url.startsWith(
        "https://github.com/Lensi-zhang/ShipWiz/releases/download/",
      ),
    );
  }
});

test("rewrites draft untagged URLs to canonical release URLs", () => {
  const { assets, manifest, platformAssets } = fixtures();
  for (const asset of assets) {
    asset.browser_download_url = asset.browser_download_url.replace(
      "/download/v0.2.6/",
      "/download/untagged-1b6932f773c14225e3e7/",
    );
  }
  for (const platform of expectedPlatforms) {
    manifest.platforms[platform].url =
      platformAssets[platform].browser_download_url;
  }

  const normalized = normalizeUpdaterManifest(
    manifest,
    assets,
    "0.2.6",
    "v0.2.6",
  );

  for (const platform of expectedPlatforms) {
    const name = platformAssets[platform].browser_download_url
      .split("/")
      .at(-1);
    assert.equal(
      normalized.platforms[platform].url,
      `https://github.com/Lensi-zhang/ShipWiz/releases/download/v0.2.6/${name}`,
    );
  }
  assert.deepEqual(
    normalizeUpdaterManifest(normalized, assets, "0.2.6", "v0.2.6"),
    normalized,
  );
});

test("rejects an unknown asset URL", () => {
  const { assets, manifest } = fixtures();
  manifest.platforms["windows-x86_64"].url =
    "https://api.github.com/repos/Lensi-zhang/ShipWiz/releases/assets/9999";
  assert.throws(
    () => normalizeUpdaterManifest(manifest, assets, "0.2.6", "v0.2.6"),
    /Unknown updater asset for windows-x86_64/,
  );
});

test("rejects missing and unexpected platforms", () => {
  const missing = fixtures();
  delete missing.manifest.platforms["darwin-aarch64-app"];
  assert.throws(
    () =>
      normalizeUpdaterManifest(
        missing.manifest,
        missing.assets,
        "0.2.6",
        "v0.2.6",
      ),
    /missing: darwin-aarch64-app/,
  );

  const extra = fixtures();
  extra.manifest.platforms["linux-aarch64"] = {
    signature: "signature",
    url: extra.assets[2].url,
  };
  assert.throws(
    () =>
      normalizeUpdaterManifest(extra.manifest, extra.assets, "0.2.6", "v0.2.6"),
    /unexpected: linux-aarch64/,
  );
});

test("rejects duplicate asset mappings", () => {
  const { assets, manifest } = fixtures();
  assets.push({
    url: assets[0].url,
    browser_download_url:
      "https://github.com/Lensi-zhang/ShipWiz/releases/download/v0.2.6/other.tar.gz",
  });
  assert.throws(
    () => normalizeUpdaterManifest(manifest, assets, "0.2.6", "v0.2.6"),
    /Duplicate asset mapping/,
  );
});

test("rejects a version mismatch", () => {
  const { assets, manifest } = fixtures();
  assert.throws(
    () => normalizeUpdaterManifest(manifest, assets, "0.2.7", "v0.2.7"),
    /does not match/,
  );
});

test("rejects an empty signature", () => {
  const { assets, manifest } = fixtures();
  manifest.platforms["linux-x86_64-deb"].signature = "  ";
  assert.throws(
    () => normalizeUpdaterManifest(manifest, assets, "0.2.6", "v0.2.6"),
    /linux-x86_64-deb signature must be a non-empty string/,
  );
});
