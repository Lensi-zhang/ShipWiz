#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";

const root = new URL("../", import.meta.url);
const semverPattern =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$/;
const usage =
  "Usage: node scripts/version.mjs check [version] | bump <version>";

function assertSemver(version, file) {
  if (typeof version !== "string" || !semverPattern.test(version)) {
    throw new Error(
      `${file}: invalid semantic version ${JSON.stringify(version)}`,
    );
  }
}

function oneMatch(text, pattern, description, file) {
  const matches = [...text.matchAll(pattern)];
  if (matches.length !== 1) {
    throw new Error(
      `${file}: expected one ${description}, found ${matches.length}`,
    );
  }
  return matches[0];
}

function tomlEntry(file, headerPattern) {
  return {
    file,
    async read() {
      const text = await readFile(new URL(file, root), "utf8");
      const headers = [...text.matchAll(headerPattern)];
      const packages = headers.map((header) => {
        const bodyStart = header.index + header[0].length;
        const nextHeader = /^\[.*\][ \t]*\r?$/m.exec(text.slice(bodyStart));
        const bodyEnd = nextHeader ? bodyStart + nextHeader.index : text.length;
        return { bodyStart, body: text.slice(bodyStart, bodyEnd) };
      });
      const shipwizPackages = packages.filter(({ body }) => {
        const names = [
          ...body.matchAll(/^name[ \t]*=[ \t]*"([^"]+)"[ \t]*\r?$/gm),
        ];
        return names.length === 1 && names[0][1] === "shipwiz";
      });
      if (shipwizPackages.length !== 1) {
        throw new Error(
          `${file}: expected one shipwiz package, found ${shipwizPackages.length}`,
        );
      }

      const { bodyStart, body } = shipwizPackages[0];
      const versionMatch = oneMatch(
        body,
        /^version[ \t]*=[ \t]*"([^"]+)"[ \t]*\r?$/gm,
        "shipwiz package version",
        file,
      );
      const version = versionMatch[1];
      assertSemver(version, file);
      const quotedVersion = `"${version}"`;
      const versionStart =
        bodyStart + versionMatch.index + versionMatch[0].indexOf(quotedVersion);

      return {
        version,
        update(nextVersion) {
          return (
            text.slice(0, versionStart) +
            `"${nextVersion}"` +
            text.slice(versionStart + quotedVersion.length)
          );
        },
      };
    },
  };
}

function jsonEntry(file, expectedName, select) {
  return {
    file,
    async read() {
      const text = await readFile(new URL(file, root), "utf8");
      const document = JSON.parse(text);
      if (
        document.name !== expectedName &&
        document.productName !== expectedName
      ) {
        throw new Error(`${file}: expected package ${expectedName}`);
      }
      const selected = select(document, file);
      assertSemver(selected.version, file);

      return {
        version: selected.version,
        update(nextVersion) {
          selected.update(nextVersion);
          const indent = text.match(/^[\t ]+(?=")/m)?.[0] ?? "  ";
          const newline = text.includes("\r\n") ? "\r\n" : "\n";
          const trailingNewline = text.endsWith("\n") ? newline : "";
          return (
            JSON.stringify(document, null, indent).replace(/\n/g, newline) +
            trailingNewline
          );
        },
      };
    },
  };
}

function rootVersion(document) {
  return {
    version: document.version,
    update(version) {
      document.version = version;
    },
  };
}

const entries = [
  tomlEntry("Cargo.toml", /^\[package\][ \t]*\r?$/gm),
  tomlEntry("Cargo.lock", /^\[\[package\]\][ \t]*\r?$/gm),
  jsonEntry("package.json", "shipwiz", rootVersion),
  jsonEntry("package-lock.json", "shipwiz", (document, file) => {
    const rootPackage = document.packages?.[""];
    if (!rootPackage || rootPackage.name !== "shipwiz") {
      throw new Error(`${file}: missing the shipwiz root package`);
    }
    if (document.version !== rootPackage.version) {
      throw new Error(`${file}: root and packages[""] versions differ`);
    }
    return {
      version: document.version,
      update(version) {
        document.version = version;
        rootPackage.version = version;
      },
    };
  }),
  jsonEntry("npm/package.json", "shipwiz-cli", rootVersion),
  jsonEntry("tauri.conf.json", "ShipWiz", (document) => ({
    version: document.version,
    update(version) {
      document.version = version;
    },
  })),
];

async function readVersions() {
  return Promise.all(
    entries.map(async (entry) => ({
      file: entry.file,
      ...(await entry.read()),
    })),
  );
}

function checkVersions(versions, expectedVersion) {
  const expected = expectedVersion ?? versions[0].version;
  if (expectedVersion) assertSemver(expectedVersion, "argument");
  const mismatches = versions.filter(({ version }) => version !== expected);
  if (mismatches.length) {
    const details = versions
      .map(({ file, version }) => `  ${file}: ${version}`)
      .join("\n");
    throw new Error(
      `expected every ShipWiz version to be ${expected}:\n${details}`,
    );
  }
  return expected;
}

async function main() {
  const [action, version, ...extra] = process.argv.slice(2);
  if (
    !["bump", "check"].includes(action) ||
    extra.length ||
    (action === "bump" && !version)
  ) {
    throw new Error(usage);
  }

  const versions = await readVersions();
  if (action === "check") {
    const checked = checkVersions(versions, version);
    console.log(`Version check passed: ${checked}`);
    for (const { file, version: current } of versions) {
      console.log(`  ${file}: ${current}`);
    }
    return;
  }

  assertSemver(version, "argument");
  const current = checkVersions(versions);
  if (current === version) {
    console.log(`ShipWiz is already at ${version}; no files changed.`);
    return;
  }

  await Promise.all(
    versions.map(async ({ file, update }) => {
      await writeFile(new URL(file, root), update(version), "utf8");
      console.log(`Updated ${file} to ${version}`);
    }),
  );
  console.log(`Bumped ShipWiz from ${current} to ${version}`);
}

try {
  await main();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  console.error(usage);
  process.exitCode = 1;
}
