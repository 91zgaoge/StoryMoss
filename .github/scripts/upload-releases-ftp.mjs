#!/usr/bin/env node
/**
 * Upload Tauri release artifacts to the website via FTP.
 *
 * Environment variables:
 *   FTP_HOST        (default: storymoss.top)
 *                     Supports plain host, host:port, or URL forms such as
 *                     ftp://host:port. When a port is present in FTP_HOST it
 *                     is used unless FTP_PORT is also set.
 *   FTP_USER        (required)
 *   FTP_PASS        (required)
 *   FTP_PORT        (default: 21)
 *   FTP_REMOTE_DIR  (default: /releases)
 *
 * Usage:
 *   node .github/scripts/upload-releases-ftp.mjs <source-dir>
 *   node .github/scripts/upload-releases-ftp.mjs --cleanup-only   # 仅按保留策略清理
 *   node .github/scripts/upload-releases-ftp.mjs --list           # 仅列出远程目录内容
 */

// basic-ftp is a dependency of landing/; resolve it relatively so this script
// can be run from the repo root without requiring a root-level node_modules.
import { Client } from "../../landing/node_modules/basic-ftp/dist/index.js";
import { config } from "../../landing/node_modules/dotenv/lib/main.js";
import { readdir, readFile, writeFile } from "node:fs/promises";
import { join, relative, resolve } from "node:path";

config();

const RELEASE_FILES = [
  "latest.json",
  /^StoryMoss_.*\.msi$/,
  /^StoryMoss_.*\.msi\.sig$/,
  /^StoryMoss_.*\.dmg$/,
  /^StoryMoss_.*\.app\.tar\.gz$/,
  /^StoryMoss_.*\.app\.tar\.gz\.sig$/,
  /^StoryMoss_.*\.AppImage$/,
  /^StoryMoss_.*\.AppImage\.sig$/,
  // v0.59.0：补 .deb——latest.json 含 linux-x86_64-deb 平台条目，
  // 白名单漏掉会让 deb 用户的应用内更新拿到 404。
  /^StoryMoss_.*\.deb$/,
  /^StoryMoss_.*\.deb\.sig$/,
];

function matchesReleaseFile(name) {
  return RELEASE_FILES.some((pattern) =>
    typeof pattern === "string" ? name === pattern : pattern.test(name),
  );
}

/**
 * Parse FTP_HOST into { host, port }.
 * Accepts: host | host:port | ftp://host | ftp://host:port
 * Explicit FTP_PORT environment variable takes precedence.
 */
function parseFtpHost(rawHost, rawPort) {
  let host = rawHost || "storymoss.top";
  let port = rawPort ? parseInt(rawPort, 10) : 21;

  // Strip ftp:// or ftps:// scheme if present.
  const schemeMatch = host.match(/^ftps?:\/\/(.+)$/i);
  if (schemeMatch) {
    host = schemeMatch[1];
  }

  // If host still contains a port, extract it unless FTP_PORT was explicitly set.
  const portMatch = host.match(/^([^:\]]+):(\d+)$/);
  if (portMatch) {
    host = portMatch[1];
    if (!rawPort) {
      port = parseInt(portMatch[2], 10);
    }
  }

  return { host, port };
}

const WEBSITE_RELEASES_URL =
  process.env.WEBSITE_RELEASES_URL || "https://storymoss.top/releases";

/**
 * Number of recent release versions to retain on the website.
 * Older versioned artifacts are deleted after each upload to prevent
 * the hosting space from growing indefinitely.
 */
const RETENTION_COUNT = parseInt(
  process.env.RELEASE_RETENTION_COUNT || "5",
  10,
);

/**
 * Rewrite the updater manifest so that binary download URLs point to the
 * website source instead of GitHub Releases. GitHub Releases keeps the
 * original manifest as the fallback endpoint.
 */
async function rewriteLatestJsonForWebsite(latestPath) {
  const content = JSON.parse(await readFile(latestPath, "utf8"));
  if (!content.platforms) return;

  for (const platform of Object.values(content.platforms)) {
    const url = platform.url;
    if (typeof url !== "string") continue;
    const fileName = url.split("/").pop();
    if (!fileName) continue;
    platform.url = `${WEBSITE_RELEASES_URL}/${fileName}`;
  }

  await writeFile(latestPath, JSON.stringify(content, null, 2));
  console.log(
    `📝 Rewrote latest.json download URLs to ${WEBSITE_RELEASES_URL}`,
  );
}

async function* walk(dir) {
  const entries = await readdir(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      yield* walk(fullPath);
    } else if (matchesReleaseFile(entry.name)) {
      yield fullPath;
    }
  }
}

/**
 * Parse a version string like "0.30.27" into a comparable numeric tuple.
 */
function parseVersion(version) {
  const parts = version.split(".").map((part) => parseInt(part, 10));
  return [parts[0] || 0, parts[1] || 0, parts[2] || 0];
}

function compareVersions(a, b) {
  const av = parseVersion(a);
  const bv = parseVersion(b);
  for (let i = 0; i < 3; i++) {
    if (av[i] !== bv[i]) return av[i] - bv[i];
  }
  return 0;
}

/**
 * Extract the embedded StoryMoss version from a release filename.
 * Examples:
 *   StoryMoss_0.30.27_amd64.deb        -> 0.30.27
 *   StoryMoss_0.30.27_x64_zh-CN.msi    -> 0.30.27
 *   StoryMoss_aarch64.app.tar.gz       -> null
 *   latest.json                        -> null
 */
function extractVersionFromFileName(name) {
  const match = name.match(/^StoryMoss_(\d+\.\d+\.\d+)_/);
  return match ? match[1] : null;
}

/**
 * Clean up old versioned release artifacts on the FTP server, retaining only
 * the most recent `RETENTION_COUNT` versions. Unversioned files (e.g. the
 * shared macOS app.tar.gz and latest.json) are never removed.
 */
async function cleanupOldReleases(client, remoteDir) {
  if (!Number.isFinite(RETENTION_COUNT) || RETENTION_COUNT <= 0) {
    console.log("⏭️  RELEASE_RETENTION_COUNT invalid; skipping cleanup");
    return;
  }

  const remoteFiles = await client.list(remoteDir);
  const filesByVersion = new Map();
  const unversioned = [];

  for (const file of remoteFiles) {
    // basic-ftp FileType: Unknown=0, File=1, Directory=2, SymbolicLink=3
    if (file.type === 2 || file.type === "directory") continue; // skip directories
    const version = extractVersionFromFileName(file.name);
    if (version) {
      if (!filesByVersion.has(version)) filesByVersion.set(version, []);
      filesByVersion.get(version).push(file.name);
    } else if (matchesReleaseFile(file.name)) {
      unversioned.push(file.name);
    }
  }

  if (filesByVersion.size <= RETENTION_COUNT) {
    console.log(
      `🧹 Found ${filesByVersion.size} version(s) on server (retention: ${RETENTION_COUNT}); nothing to clean up`,
    );
    return;
  }

  const sortedVersions = Array.from(filesByVersion.keys())
    .sort(compareVersions)
    .reverse();
  const versionsToKeep = new Set(sortedVersions.slice(0, RETENTION_COUNT));
  const versionsToDelete = sortedVersions.slice(RETENTION_COUNT);

  console.log(
    `🧹 Retaining ${RETENTION_COUNT} newest version(s): ${Array.from(versionsToKeep).join(", ")}`,
  );
  console.log(
    `🗑️  Deleting ${versionsToDelete.length} old version(s): ${versionsToDelete.join(", ")}`,
  );

  for (const version of versionsToDelete) {
    for (const fileName of filesByVersion.get(version)) {
      console.log(`  🗑️  ${fileName}`);
      await client.remove(join(remoteDir, fileName));
    }
  }

  console.log("✅ Old release cleanup complete");
}

/**
 * List everything in the remote releases directory, sorted by size (desc),
 * with a grand total. Used to diagnose disk-full situations where the
 * retention cleanup cannot see what is actually occupying space.
 */
async function listRemote(client, remoteDir) {
  const remoteFiles = await client.list(remoteDir);
  const rows = [];
  let total = 0;
  for (const file of remoteFiles) {
    const isDir = file.type === 2 || file.type === "directory";
    const size = isDir ? 0 : file.size;
    total += size;
    rows.push({ name: file.name, size, isDir, date: file.rawModifiedAt });
  }
  rows.sort((a, b) => b.size - a.size);

  console.log(`📂 Contents of ${remoteDir} (${rows.length} entries):`);
  for (const row of rows) {
    const sizeStr = row.isDir
      ? "<dir>"
      : `${(row.size / 1024 / 1024).toFixed(2)} MB`;
    console.log(`  ${sizeStr.padStart(10)}  ${row.date || ""}  ${row.name}`);
  }
  console.log(
    `📦 Total: ${(total / 1024 / 1024).toFixed(2)} MB across ${rows.length} entries`,
  );
}

async function main() {
  // --cleanup-only：只执行保留策略清理，不上传（用于磁盘满时手动救火，
  // 见 .github/workflows/cleanup-releases.yml）
  const cleanupOnly = process.argv.includes("--cleanup-only");
  // --list：只列出远程目录内容（磁盘满诊断），不清理也不上传
  const listOnly = process.argv.includes("--list");
  const positional = process.argv.slice(2).filter((a) => !a.startsWith("--"));
  const sourceDir = resolve(positional[0] || "src-tauri/target/release/bundle");
  const { host, port } = parseFtpHost(
    process.env.FTP_HOST,
    process.env.FTP_PORT,
  );
  const user = process.env.FTP_USER;
  const password = process.env.FTP_PASS;
  const remoteDir = process.env.FTP_REMOTE_DIR || "/releases";

  if (!user || !password) {
    console.error("❌ Missing FTP_USER or FTP_PASS environment variable");
    process.exit(1);
  }

  const client = new Client();
  client.ftp.verbose = process.env.FTP_VERBOSE === "true";

  try {
    console.log(`🚀 Connecting to FTP ${host}:${port}...`);
    await client.access({ host, port, user, password, secure: false });
    await client.ensureDir(remoteDir);

    if (listOnly) {
      await listRemote(client, remoteDir);
      return;
    }

    // 上传前先按保留策略清理：磁盘满（552 Disk full）时自愈——
    // 旧版仅在上传后清理，磁盘已满时上传先失败、清理永远轮不到。
    await cleanupOldReleases(client, remoteDir);

    if (cleanupOnly) {
      console.log("✅ Cleanup-only run complete");
      return;
    }

    const files = [];
    for await (const file of walk(sourceDir)) {
      files.push(file);
    }

    if (files.length === 0) {
      console.warn("⚠️ No release artifacts found in", sourceDir);
      process.exit(0);
    }

    // Upload latest.json last so clients never see a manifest before its binaries.
    files.sort((a, b) => {
      const aIsManifest = a.endsWith("latest.json") ? 1 : 0;
      const bIsManifest = b.endsWith("latest.json") ? 1 : 0;
      return aIsManifest - bIsManifest;
    });

    const latestPath = files.find((f) => f.endsWith("latest.json"));
    if (latestPath) {
      await rewriteLatestJsonForWebsite(latestPath);
    }

    for (const localPath of files) {
      const fileName = localPath.split("/").pop().split("\\").pop();
      console.log(`  ⬆️  ${fileName}`);
      await client.uploadFrom(localPath, fileName);
    }

    console.log(`✅ Uploaded ${files.length} file(s) to ${host}${remoteDir}`);
  } catch (err) {
    console.error("❌ FTP upload failed:", err.message);
    process.exit(1);
  } finally {
    client.close();
  }
}

main();
