# StoryMoss 自动更新源迁移到 storymoss.top 设计文档

## 背景

当前 StoryMoss 桌面端的自动更新依赖 GitHub Releases：

- `src-tauri/tauri.conf.json` 的 updater endpoint 指向：
  `https://github.com/91zgaoge/StoryMoss/releases/latest/download/latest.json`
- CI 构建产物（`.msi`、`.dmg`、`.AppImage` 及对应 `.sig`、`latest.json`）通过 `tauri-apps/tauri-action` 发布到 GitHub Release。
- 落地页 `landing/src/components/DownloadButton.tsx` 的下载按钮也指向 GitHub Releases。

为了降低对 GitHub 的依赖、提升国内访问稳定性，并统一品牌出口，需要将自动更新主源迁移到官网 `storymoss.top`，通过 FTP 上传构建产物，应用内通过 HTTPS 拉取更新清单。

## 目标

1. 将应用内自动更新主源从 GitHub Releases 切换到 `storymoss.top/releases/latest.json`。
2. 保留 GitHub Releases 作为回退源，确保主源不可用时仍能更新。
3. CI 构建完成后自动通过 FTP 将更新产物同步到 `storymoss.top/releases/`。
4. 落地页下载按钮同步指向 `storymoss.top/releases/...`。
5. 保留现有的 Tauri 签名验证机制，确保更新安全。

## 方案概述

采用 **Tauri 原生多 endpoints + CI FTP 同步** 方案：

- 不改动 `tauri-plugin-updater` 的核心调用逻辑。
- 仅修改 `tauri.conf.json` 的 `endpoints` 为数组，主源在前、回退源在后。
- 在 CI 中新增 FTP 上传步骤，将构建产物推到网站服务器。
- 修改错误提示和落地页链接，完成品牌出口统一。

## 数据流

```
开发者推送 tag v* -> GitHub Actions
   |
   v
Tauri 构建各平台安装包 + latest.json
   |
   v
发布到 GitHub Release（保留）
   |
   v
FTP 上传到 storymoss.top/releases/
   |
   v
应用启动 / 每 4 小时 check_update
   |
   v
Tauri updater 请求 https://storymoss.top/releases/latest.json
   |-- 失败则自动回退到 GitHub Releases
   |
   v
下载安装包 -> 验证签名 -> 安装并重启
```

## 具体改动

### 1. `src-tauri/tauri.conf.json`

将 `plugins.updater.endpoints` 从单 GitHub URL 改为数组：

```json
"endpoints": [
  "https://storymoss.top/releases/latest.json",
  "https://github.com/91zgaoge/StoryMoss/releases/latest/download/latest.json"
]
```

Tauri updater 会按顺序尝试，主源失败自动使用回退源。

### 2. `src-tauri/src/updater/mod.rs`

- 更新模块顶部注释，说明下载源已改为 `storymoss.top/releases/latest.json`，GitHub 作为回退。
- 修改 `format_updater_error` 的错误文案，将“无法从 GitHub 读取 latest.json”改为“无法从 storymoss.top 读取更新清单”，并附带两个源的链接。
- 同步更新对应单元测试的断言。

### 3. `.github/workflows/build.yml`

在 `tauri-build` job 之后新增 `upload-to-website` job：

- 依赖 `tauri-build` 完成。
- 仅在 tag push（stable）和普通 push（nightly）时执行，PR 不执行。
- 复用 `basic-ftp`（项目 `landing/` 已安装）。
- 上传以下文件到 FTP `/releases/` 目录：
  - `latest.json`
  - Windows: `StoryMoss_<version>_x64_zh-CN.msi` + `.msi.sig`
  - macOS: `StoryMoss_<version>_aarch64.dmg`、`.app.tar.gz` + `.app.tar.gz.sig`
  - Linux: `StoryMoss_<version>_amd64.AppImage` + `.AppImage.sig`
- 上传顺序：先上传安装包和 `.sig`，最后上传 `latest.json`，避免客户端读到不完整状态。
- FTP 配置从 GitHub Secrets 读取：
  - `FTP_HOST`（默认 `storymoss.top`）
  - `FTP_USER`
  - `FTP_PASS`
  - `FTP_PORT`（默认 21）

### 4. 新增 `.github/scripts/upload-releases-ftp.js`

封装 FTP 上传逻辑，供 CI 调用。脚本特性：

- 接收 `--source-dir` 参数（构建产物目录）。
- 通过环境变量读取 FTP 配置。
- 使用 `basic-ftp` 连接并上传文件。
- 保持远程目录结构为 `/releases/`（扁平，不保留子目录）。
- 输出上传文件列表和结果。

### 5. `landing/src/components/DownloadButton.tsx`

将下载基础地址从 GitHub Releases 改为 `storymoss.top/releases`：

```ts
const RELEASE_BASE = 'https://storymoss.top/releases';
```

文件名格式保持不变，因为 CI 产出的文件名与 GitHub Release 一致。

### 6. `landing/src/components/__tests__/DownloadButton.test.tsx`

同步更新测试断言中的 URL，从 `github.com/91zgaoge/StoryMoss/releases/...` 改为 `storymoss.top/releases/...`。

### 7. 文档更新

- 更新 `README.md` 中关于自动更新源和下载链接的描述。
- 更新 `docs/USER_GUIDE.md` 中「无法读取 latest.json」的排查说明。
- 更新 `.claude/skills/sf-run-and-operate/SKILL.md` 中 updater endpoint 的描述。

## 安全与签名

- 保留 `tauri.conf.json` 中的 `pubkey` 不变。
- CI 继续通过 `TAURI_SIGNING_PRIVATE_KEY` 对更新包签名。
- 应用下载后仍由 Tauri updater 验证签名，防止中间人攻击。

## 错误处理

- Tauri updater 原生支持多 endpoint 自动回退，无需额外代码。
- FTP 上传失败不阻塞 GitHub Release 创建，但 CI 会标红并输出警告。
- `format_updater_error` 的错误信息会同时提示主源和回退源，方便排查。

## 验证计划

1. **CI 验证**：合并后推送一个测试 tag，确认 `upload-to-website` job 成功，且 `https://storymoss.top/releases/latest.json` 可访问。
2. **本地更新检查**：运行桌面应用，手动触发「检查更新」，确认请求的是 `storymoss.top/releases/latest.json`。
3. **落地页测试**：运行 `landing` 测试套件，确认 `DownloadButton` 链接正确。
4. **回退验证**：临时屏蔽 storymoss.top 请求，确认应用能回退到 GitHub Releases。

## 回滚方案

若 storymoss.top 更新源出现故障：

1. 临时将 `tauri.conf.json` 的 `endpoints` 改回仅 GitHub Releases。
2. 重新发布一个补丁版本即可恢复，无需改动 CI。

## 决策记录

- **主源域名**：`storymoss.top`（用户指定）。
- **回退源**：保留 GitHub Releases（用户要求兼容）。
- **更新文件路径**：`/releases/`（用户选择）。
- **FTP 用途**：仅用于上传构建产物；应用内仍走 HTTPS 拉取（用户选择）。
- **部署方式**：CI 自动上传（用户选择）。
