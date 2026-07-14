#!/usr/bin/env node
/**
 * StoryMoss 落地页自动部署脚本
 *
 * 用法：
 *   1. 在 landing/ 目录下创建 .env 文件，填写 FTP 信息：
 *      FTP_HOST=ai.91z.net
 *      FTP_PORT=21       （可选，默认 21）
 *      FTP_USER=your-ftp-user
 *      FTP_PASS=your-ftp-password
 *      FTP_REMOTE_DIR=/  （可选，默认 FTP 根目录）
 *
 *      FTP_HOST 也支持 ftp://host:port 形式；如果 URL 中带端口，会优先使用，
 *      除非同时显式设置了 FTP_PORT。
 *
 *   2. 运行：
 *      npm run deploy
 *
 * 说明：
 *   - 脚本会先执行 npm run build 生成 dist/
 *   - 然后通过 FTP 把 dist/ 下的文件上传到远程目录
 *   - 不会删除远程已有的其他文件，仅覆盖同名文件
 */

import { Client } from 'basic-ftp';
import { config } from 'dotenv';
import { readdir } from 'node:fs/promises';
import { join, relative } from 'node:path';

config();

/**
 * Parse FTP_HOST into { host, port }.
 * Accepts: host | host:port | ftp://host | ftp://host:port
 * Explicit FTP_PORT environment variable takes precedence.
 */
function parseFtpHost(rawHost, rawPort) {
  let host = rawHost;
  let port = rawPort ? parseInt(rawPort, 10) : 21;

  const schemeMatch = host.match(/^ftps?:\/\/(.+)$/i);
  if (schemeMatch) {
    host = schemeMatch[1];
  }

  const portMatch = host.match(/^([^:\]]+):(\d+)$/);
  if (portMatch) {
    host = portMatch[1];
    if (!rawPort) {
      port = parseInt(portMatch[2], 10);
    }
  }

  return { host, port };
}

const required = ['FTP_HOST', 'FTP_USER', 'FTP_PASS'];
for (const key of required) {
  if (!process.env[key]) {
    console.error(`❌ 缺少环境变量：${key}`);
    console.error('请在 landing/.env 文件中配置 FTP 信息');
    process.exit(1);
  }
}

const { host, port } = parseFtpHost(process.env.FTP_HOST, process.env.FTP_PORT);
const user = process.env.FTP_USER;
const password = process.env.FTP_PASS;
const remoteDir = process.env.FTP_REMOTE_DIR || '/';
const localDir = join(process.cwd(), 'dist');

async function* walk(dir) {
  const entries = await readdir(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      yield* walk(fullPath);
    } else {
      yield fullPath;
    }
  }
}

async function deploy() {
  console.log('🏗️  开始构建...');
  const { execSync } = await import('node:child_process');
  execSync('npm run build', { stdio: 'inherit', cwd: process.cwd() });

  console.log('\n🚀 开始上传到 FTP...');
  const client = new Client();
  client.ftp.verbose = process.env.FTP_VERBOSE === 'true';

  try {
    await client.access({ host, port, user, password, secure: false });
    if (remoteDir && remoteDir !== '/') {
      await client.ensureDir(remoteDir);
    }

    let uploaded = 0;
    for await (const localPath of walk(localDir)) {
      const relPath = relative(localDir, localPath);
      const remotePath = relPath.replace(/\\/g, '/');
      const remoteFolder = remotePath.split('/').slice(0, -1).join('/');

      if (remoteFolder) {
        await client.ensureDir(remoteFolder);
      }

      await client.uploadFrom(localPath, remotePath.split('/').pop());
      console.log(`  ✓ ${relPath}`);
      uploaded++;

      if (remoteFolder) {
        await client.cd('/');
      }
    }

    console.log(`\n✅ 部署完成，共上传 ${uploaded} 个文件`);
    console.log(`🌐 访问：https://${host}`);
  } catch (err) {
    console.error('\n❌ 部署失败：', err.message);
    process.exit(1);
  } finally {
    client.close();
  }
}

deploy();
