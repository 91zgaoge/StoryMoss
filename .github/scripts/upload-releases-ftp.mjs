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
 */

// basic-ftp is a dependency of landing/; resolve it relatively so this script
// can be run from the repo root without requiring a root-level node_modules.
import { Client } from '../../landing/node_modules/basic-ftp/dist/index.js';
import { config } from '../../landing/node_modules/dotenv/lib/main.js';
import { readdir } from 'node:fs/promises';
import { join, relative, resolve } from 'node:path';

config();

const RELEASE_FILES = [
  'latest.json',
  /^StoryMoss_.*\.msi$/,
  /^StoryMoss_.*\.msi\.sig$/,
  /^StoryMoss_.*\.dmg$/,
  /^StoryMoss_.*\.app\.tar\.gz$/,
  /^StoryMoss_.*\.app\.tar\.gz\.sig$/,
  /^StoryMoss_.*\.AppImage$/,
  /^StoryMoss_.*\.AppImage\.sig$/,
];

function matchesReleaseFile(name) {
  return RELEASE_FILES.some((pattern) =>
    typeof pattern === 'string' ? name === pattern : pattern.test(name)
  );
}

/**
 * Parse FTP_HOST into { host, port }.
 * Accepts: host | host:port | ftp://host | ftp://host:port
 * Explicit FTP_PORT environment variable takes precedence.
 */
function parseFtpHost(rawHost, rawPort) {
  let host = rawHost || 'storymoss.top';
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

async function main() {
  const sourceDir = resolve(process.argv[2] || 'src-tauri/target/release/bundle');
  const { host, port } = parseFtpHost(process.env.FTP_HOST, process.env.FTP_PORT);
  const user = process.env.FTP_USER;
  const password = process.env.FTP_PASS;
  const remoteDir = process.env.FTP_REMOTE_DIR || '/releases';

  if (!user || !password) {
    console.error('❌ Missing FTP_USER or FTP_PASS environment variable');
    process.exit(1);
  }

  const files = [];
  for await (const file of walk(sourceDir)) {
    files.push(file);
  }

  if (files.length === 0) {
    console.warn('⚠️ No release artifacts found in', sourceDir);
    process.exit(0);
  }

  // Upload latest.json last so clients never see a manifest before its binaries.
  files.sort((a, b) => {
    const aIsManifest = a.endsWith('latest.json') ? 1 : 0;
    const bIsManifest = b.endsWith('latest.json') ? 1 : 0;
    return aIsManifest - bIsManifest;
  });

  const client = new Client();
  client.ftp.verbose = process.env.FTP_VERBOSE === 'true';

  try {
    console.log(`🚀 Connecting to FTP ${host}:${port}...`);
    await client.access({ host, port, user, password, secure: false });
    await client.ensureDir(remoteDir);

    for (const localPath of files) {
      const fileName = localPath.split('/').pop().split('\\').pop();
      console.log(`  ⬆️  ${fileName}`);
      await client.uploadFrom(localPath, fileName);
    }

    console.log(`✅ Uploaded ${files.length} file(s) to ${host}${remoteDir}`);
  } catch (err) {
    console.error('❌ FTP upload failed:', err.message);
    process.exit(1);
  } finally {
    client.close();
  }
}

main();
