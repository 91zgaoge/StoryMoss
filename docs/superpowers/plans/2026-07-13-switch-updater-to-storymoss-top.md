# Switch StoryMoss Updater Source to storymoss.top Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate the StoryMoss desktop auto-updater from GitHub Releases to `storymoss.top/releases/` as the primary source, with GitHub Releases kept as a fallback, and update the landing page download button to point to the website source.

**Architecture:** Keep `tauri-plugin-updater` as the core update mechanism; only change the `endpoints` array in `tauri.conf.json` to prioritize `storymoss.top`. Add a CI job that FTP-uploads signed build artifacts to the website after a GitHub Release is created. Update the landing page download button and its tests to use the new website URL.

**Tech Stack:** Tauri 2.x, `tauri-plugin-updater`, GitHub Actions, `basic-ftp`, Node.js 20.

## Global Constraints

- The primary updater endpoint MUST be `https://storymoss.top/releases/latest.json`.
- GitHub Releases MUST remain as a fallback endpoint.
- Existing Tauri signing key and `pubkey` in `tauri.conf.json` MUST NOT change.
- FTP credentials come from GitHub Secrets: `FTP_HOST`, `FTP_USER`, `FTP_PASS`, optional `FTP_PORT` (default 21).
- All Rust code MUST pass `cargo +nightly fmt -- --check` and existing tests.
- All frontend code MUST pass `npm run format:check` and `npm run type-check`.

---

### Task 1: Update updater endpoints in `tauri.conf.json`

**Files:**
- Modify: `src-tauri/tauri.conf.json:94-96`

**Interfaces:**
- Consumes: None.
- Produces: `plugins.updater.endpoints` array with website first, GitHub second.

- [ ] **Step 1: Replace the single GitHub endpoint with an array**

```json
"endpoints": [
  "https://storymoss.top/releases/latest.json",
  "https://github.com/91zgaoge/StoryMoss/releases/latest/download/latest.json"
]
```

- [ ] **Step 2: Verify JSON is valid**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/src-tauri
python3 -m json.tool tauri.conf.json > /dev/null
```

Expected: No output (valid JSON).

- [ ] **Step 3: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add src-tauri/tauri.conf.json
git commit -m "config: prioritize storymoss.top updater endpoint, keep github fallback"
```

---

### Task 2: Update updater error messages and tests

**Files:**
- Modify: `src-tauri/src/updater/mod.rs:66-82`, `:1-8`
- Modify: `src-tauri/src/updater/mod.rs:200-206`

**Interfaces:**
- Consumes: `format_updater_error` signature unchanged.
- Produces: Error messages now reference `storymoss.top` and GitHub fallback.

- [ ] **Step 1: Update module comments**

Replace lines 1-8 with:

```rust
//! Updater Module - 自动更新功能
//!
//! 提供应用自动检测更新和安装的功能
//! 基于 tauri-plugin-updater
//!
//! 下载源：`plugins.updater.endpoints` → 优先 storymoss.top/releases，回退 GitHub Releases
//! 主端点：`https://storymoss.top/releases/latest.json`
//! 回退端点：`https://github.com/91zgaoge/StoryMoss/releases/latest/download/latest.json`
```

- [ ] **Step 2: Update error message formatter**

Replace `format_updater_error` with:

```rust
fn format_updater_error(err: impl std::fmt::Display) -> String {
    let msg = err.to_string();
    let lower = msg.to_lowercase();
    if lower.contains("404")
        || lower.contains("not found")
        || lower.contains("failed to fetch")
        || lower.contains("error decoding response body")
    {
        format!(
            "无法从 storymoss.top 读取更新清单（latest.json）。\
             请确认官网下载目录已包含最新版本的更新文件：\
             https://storymoss.top/releases/ 。\
             也可手动前往 GitHub Releases 下载：\
             https://github.com/91zgaoge/StoryMoss/releases/latest 。详情: {msg}"
        )
    } else {
        format!("Failed to check update: {msg}")
    }
}
```

- [ ] **Step 3: Update the unit test**

Replace the existing test around line 200 with:

```rust
    #[test]
    fn format_updater_error_mentions_storymoss_top_on_404() {
        let msg = format_updater_error(
            "error sending request for url (https://storymoss.top/releases/latest.json): 404 Not Found",
        );
        assert!(msg.contains("latest.json"));
        assert!(msg.contains("storymoss.top"));
        assert!(msg.contains("GitHub"));
    }
```

- [ ] **Step 4: Format and test Rust**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/src-tauri
cargo +nightly fmt
cargo test --lib updater::tests
```

Expected: `test result: ok` for updater tests.

- [ ] **Step 5: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add src-tauri/src/updater/mod.rs
git commit -m "refactor(updater): update error messages to reference storymoss.top primary source"
```

---

### Task 3: Create FTP upload script for CI

**Files:**
- Create: `.github/scripts/upload-releases-ftp.js`

**Interfaces:**
- Consumes: Environment variables `FTP_HOST`, `FTP_USER`, `FTP_PASS`, optional `FTP_PORT`.
- Produces: Exits 0 on success, 1 on failure.

- [ ] **Step 1: Create the script**

```javascript
#!/usr/bin/env node
/**
 * Upload Tauri release artifacts to the website via FTP.
 *
 * Environment variables:
 *   FTP_HOST        (default: storymoss.top)
 *   FTP_USER        (required)
 *   FTP_PASS        (required)
 *   FTP_PORT        (default: 21)
 *   FTP_REMOTE_DIR  (default: /releases)
 *
 * Usage:
 *   node .github/scripts/upload-releases-ftp.js <source-dir>
 */

import { Client } from 'basic-ftp';
import { config } from 'dotenv';
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
  const host = process.env.FTP_HOST || 'storymoss.top';
  const port = parseInt(process.env.FTP_PORT || '21', 10);
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
```

- [ ] **Step 2: Make the script executable**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge
chmod +x .github/scripts/upload-releases-ftp.js
```

- [ ] **Step 3: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add .github/scripts/upload-releases-ftp.js
git commit -m "ci(scripts): add FTP upload script for release artifacts"
```

---

### Task 4: Add FTP upload job to `build.yml`

**Files:**
- Modify: `.github/workflows/build.yml` (after the `tauri-build` job and before `verify-updater-manifest`)

**Interfaces:**
- Consumes: Artifacts produced by `tauri-build`.
- Produces: Uploaded files on `storymoss.top/releases/`.

- [ ] **Step 1: Add the new job after `tauri-build`**

Insert this YAML after the `tauri-build` job block and before `verify-updater-manifest`:

```yaml
  # Sync signed release artifacts to storymoss.top via FTP
  upload-to-website:
    needs: [tauri-build]
    if: github.event_name == 'push'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'
          cache-dependency-path: |
            landing/package.json
            landing/package-lock.json

      - name: Install landing dependencies
        working-directory: landing
        run: npm ci

      - name: Download build artifacts
        uses: actions/download-artifact@v4
        with:
          pattern: storymoss-*
          path: artifacts
          merge-multiple: true

      - name: Upload release artifacts to storymoss.top
        env:
          FTP_HOST: ${{ secrets.FTP_HOST }}
          FTP_PORT: ${{ secrets.FTP_PORT }}
          FTP_USER: ${{ secrets.FTP_USER }}
          FTP_PASS: ${{ secrets.FTP_PASS }}
          FTP_REMOTE_DIR: /releases
        run: node .github/scripts/upload-releases-ftp.js artifacts

      - name: Verify latest.json is accessible
        run: |
          for i in $(seq 1 30); do
            if curl -fsSIL "https://storymoss.top/releases/latest.json" >/dev/null 2>&1; then
              echo "✅ https://storymoss.top/releases/latest.json is accessible"
              exit 0
            fi
            echo "attempt ${i}/30..."
            sleep 10
          done
          echo "❌ latest.json not accessible after upload"
          exit 1
```

- [ ] **Step 2: Validate workflow YAML syntax**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/build.yml'))" && echo "YAML OK"
```

Expected: `YAML OK`.

- [ ] **Step 3: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add .github/workflows/build.yml
git commit -m "ci: add FTP upload job to sync release artifacts to storymoss.top"
```

---

### Task 5: Update landing page download button

**Files:**
- Modify: `landing/src/components/DownloadButton.tsx:4-5`, `:8-10`, `:38`

**Interfaces:**
- Consumes: Current version string from `package.json` indirectly (still hard-coded as before).
- Produces: Download URLs now point to `storymoss.top/releases/`.

- [ ] **Step 1: Replace GitHub release base URL**

Replace lines 4-11 with:

```ts
const RELEASE_BASE = 'https://storymoss.top/releases';

const ASSETS = {
  mac: `${RELEASE_BASE}/StoryMoss_0.26.59_aarch64.dmg`,
  windows: `${RELEASE_BASE}/StoryMoss_0.26.59_x64_zh-CN.msi`,
  linux: `${RELEASE_BASE}/StoryMoss_0.26.59_amd64.AppImage`,
};
```

- [ ] **Step 2: Update fallback URL**

Replace line 38 (`return 'https://github.com/91zgaoge/StoryMoss/releases/latest';`) with:

```ts
return 'https://storymoss.top/releases/';
```

- [ ] **Step 3: Format and type-check landing**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/landing
npm run format:check
npm run type-check
```

Expected: Both pass.

- [ ] **Step 4: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add landing/src/components/DownloadButton.tsx
git commit -m "feat(landing): point download button to storymoss.top releases"
```

---

### Task 6: Update landing page download button tests

**Files:**
- Modify: `landing/src/components/__tests__/DownloadButton.test.tsx`

**Interfaces:**
- Consumes: `downloadUrl` and `DownloadButton` from `DownloadButton.tsx`.
- Produces: Tests assert against `storymoss.top` URLs.

- [ ] **Step 1: Update test expectations**

Find all occurrences of `github.com/91zgaoge/StoryMoss/releases` in the test file and replace with `storymoss.top/releases`.

For example, replace:
```ts
expect(link.href).toContain('github.com/91zgaoge/StoryMoss/releases/download/v0.26.59');
```
with:
```ts
expect(link.href).toContain('storymoss.top/releases/StoryMoss_0.26.59');
```

And replace:
```ts
expect(downloadUrl('unknown')).toBe('https://github.com/91zgaoge/StoryMoss/releases/latest');
```
with:
```ts
expect(downloadUrl('unknown')).toBe('https://storymoss.top/releases/');
```

- [ ] **Step 2: Run landing tests**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/landing
npm run test:run
```

Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add landing/src/components/__tests__/DownloadButton.test.tsx
git commit -m "test(landing): update download button tests for storymoss.top URLs"
```

---

### Task 7: Update project documentation

**Files:**
- Modify: `README.md:14`, `:434-437` (updater mentions)
- Modify: `docs/USER_GUIDE.md:622` (troubleshooting latest.json)
- Modify: `.claude/skills/sf-run-and-operate/SKILL.md:61`

**Interfaces:**
- Consumes: None.
- Produces: Docs reflect new updater source.

- [ ] **Step 1: Update README updater references**

Replace the mention of GitHub Releases as the updater source with:

```markdown
**上一版 v0.26.48** 修复自动更新——开启 `createUpdaterArtifacts`，CI 产出 `latest.json`/签名包并同步到 `https://storymoss.top/releases/`；应用内优先从官网检查更新，GitHub Releases 保留为回退源；Linux 补 AppImage。
```

Update the install-from-source section if it mentions updater endpoint; otherwise leave it.

- [ ] **Step 2: Update USER_GUIDE troubleshooting**

Replace the existing latest.json troubleshooting line with:

```markdown
若提示无法读取 `latest.json`，请确认官网 `https://storymoss.top/releases/latest.json` 已包含最新版本，或手动前往 [GitHub Releases](https://github.com/91zgaoge/StoryMoss/releases/latest) 下载。
```

- [ ] **Step 3: Update sf-run-and-operate skill**

Replace the endpoint description in `.claude/skills/sf-run-and-operate/SKILL.md:61` with:

```markdown
CI 在 tag push 时触发 `tauri-build` 的 stable 分支，发布 GitHub Release（含 `.msi`/`.dmg`/`.deb`/`.AppImage` 及 `.sig` / `.app.tar.gz`）并同步到 `https://storymoss.top/releases/`；应用内升级器优先读取 `https://storymoss.top/releases/latest.json`，GitHub Releases 作为回退源。前提：`bundle.createUpdaterArtifacts=true` + Secret `TAURI_SIGNING_PRIVATE_KEY`；`verify-updater-manifest` job 会在 tag 构建后校验清单存在。
```

- [ ] **Step 4: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add README.md docs/USER_GUIDE.md .claude/skills/sf-run-and-operate/SKILL.md
git commit -m "docs: update updater source references to storymoss.top"
```

---

### Task 8: Final verification

**Files:**
- All files modified above.

**Interfaces:**
- Consumes: None.
- Produces: Confirmed working tree state.

- [ ] **Step 1: Run formatting and type checks**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/src-tauri
cargo +nightly fmt -- --check
cargo check
```

Expected: No formatting errors, `cargo check` succeeds.

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/landing
npm run format:check
npm run type-check
npm run test:run
```

Expected: All pass.

- [ ] **Step 2: Confirm git log**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge
git log --oneline -10
```

Expected: See commits for Tasks 1-7.

- [ ] **Step 3: Final summary**

Report back:
- `src-tauri/tauri.conf.json` now prioritizes `storymoss.top/releases/latest.json`.
- `src-tauri/src/updater/mod.rs` error messages reference the new primary source.
- CI will FTP-upload release artifacts to `storymoss.top/releases/`.
- Landing page download button points to `storymoss.top/releases/`.
- Tests and docs are updated.

## Self-Review

**Spec coverage:**
- Primary endpoint set to `storymoss.top/releases/latest.json`: Task 1.
- GitHub fallback preserved: Task 1.
- FTP upload of build artifacts: Tasks 3-4.
- Landing page download button updated: Task 5.
- Existing signing mechanism unchanged: Global Constraints.
- Tests updated: Task 6.
- Docs updated: Task 7.

**Placeholder scan:** No TBD/TODO/fill-in placeholders; every step shows exact code or commands.

**Type consistency:** `basic-ftp` Client API matches the existing `landing/scripts/deploy.js` usage. URL strings and file name patterns match current CI artifact names.
