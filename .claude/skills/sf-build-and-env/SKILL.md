---
name: sf-build-and-env
description: 从零重建 StoryForge 开发与构建环境，含已知陷阱。何时加载：首次 clone 后要跑起来、换机器、CI 本地复现、`cargo tauri dev/build` 报环境错、前端起不来、跨平台构建失败、或被问“怎么构建/怎么跑”时。
---

# StoryForge 构建与环境

## 前置依赖

| 工具 | 版本 | 来源 | 备注 |
| --- | --- | --- | --- |
| Node.js | 20 LTS | nodejs.org | CI 用 20；本地 18 也勉强可用但未保证 |
| Rust | **1.95.0（固定）** | `rust-toolchain.toml` 自动固定 | `rustup` 会自动装；不要手动 `rustup default` 改版本 |
| protoc | 任意 | `arduino/setup-protoc@v3` 或系统安装 | Lancedb/embedding 依赖；缺失会编译失败 |
| Tauri CLI | `@tauri-apps/cli ^2.10.1` | `src-frontend` devDependency | 用 `npx tauri` 或 `cargo tauri` 均可 |
| Playwright | latest | 根 `package.json` | 仅 E2E 需要 |

Ubuntu 额外系统包（CI 已验证）：
```bash
sudo apt-get install -y libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf
```

## 从零起步

```bash
# 1. clone
git clone https://github.com/91zgaoge/StoryForge.git
cd StoryForge

# 2. 前端依赖
cd src-frontend && npm ci      # 用 ci 而非 install，保证与 lockfile 一致

# 3. 仅前端调试（浏览器，无 Tauri 后端，IPC 会挂起——见陷阱）
npm run dev                    # http://localhost:5173/

# 4. 桌面应用（自动起前端 dev server + 开窗）
cd ..
npx tauri dev                  # 或 cd src-tauri && cargo tauri dev

# 5. 生产构建（本平台安装包）
cd src-tauri && cargo tauri build
```

> README 写 `npm install -g @tauri-apps/cli`，但 `src-frontend` 已把 `@tauri-apps/cli` 列为 devDependency，**用 `npx tauri` 即可，无需全局安装**。

## 已知陷阱（每个都真实踩过）

| 症状 | 根因 | 解决 |
| --- | --- | --- |
| `cargo tauri dev` 加载陈旧页面 / 崩溃 | `tauri.conf.json` 的 `devUrl` 必须指向 `http://localhost:5173`（dev server）；若指向 `dist` 会加载旧产物 | 确认 `devUrl`；先起 `npm run dev` 再 `tauri dev` |
| `state() called before manage() for Arc<dyn VectorStore>` 启动 panic | Tauri State 注入顺序错 | v0.23.6 已修：`LanceVectorStore` 创建与 `app.manage` 提前到依赖组件之前。若复现说明有人改了 `lib.rs`/`setup` 顺序 |
| 启动 panic / Windows 闪退（Issue #4） | `init_db` 在不可写目录失败；`GatewayExecutor::new` 曾通过 `state::<DbPool>()` 读未 manage 的 pool | v0.26.16 已修：`setup` 显式传 pool，仅在 pool 可用时初始化网关。复现→检查 app data 目录权限 |
| Windows MSI 构建挂掉 / `light.exe` 失败 | 迁移文件名含中文/全角标点/破折号或过长 | v0.26.21：迁移文件名必须 ASCII 短名 `V###__*.sql` |
| macOS 公证失败 | Apple Developer 协议过期 / 证书密码带尾随空格 | 续签协议；GitHub Secret `APPLE_CERTIFICATE_PASSWORD` 必须正好 28 字符无尾随空白 |
| `cargo +nightly fmt -- --check` 跨平台不一致 | `format_strings` 在 macOS/Windows nightly 行为不同 | `rustfmt.toml` 已注释禁用 `format_strings`；**不要重新启用** |
| LanceDB 持久化构建冲突 | Arrow 依赖与当前工具链冲突 | 长期 blocked；当前用 SQLite 向量持久化兜底 |
| 前端 E2E 在 settings 页 IPC 挂起 | E2E 跑在 Vite dev server 上，缺真实 Tauri 后端 | CI 中 E2E `continue-on-error: true`；本地 E2E 仅测前端行为 |
| 推理模型生成空 `{}` JSON | 思考链里的花括号被 `find('{')` 误判 | v0.23.49：`strip_reasoning_blocks` + `extract_first_json_object` 跳过空对象 |

## 跨平台构建产物

`cargo tauri build` 在本机产出（macOS）：`src-tauri/target/release/bundle/dmg/StoryForge_<ver>_aarch64.dmg`。
CI 矩阵：Ubuntu→`.deb`、Windows→`.msi`、macOS→`.dmg`（见 `.github/workflows/build.yml` 的 `tauri-build` job）。

## 何时 NOT 用本技能

- 跑测试的具体命令与门槛 → `sf-validation-and-qa`。
- 运行时日志/产物落点 → `sf-run-and-operate`。
- CI 失败排查 → `sf-debugging-playbook`。

## 出处与维护

- 重验证命令：
  - `cat rust-toolchain.toml`（Rust 版本是否仍 1.95.0）
  - `node -v`（是否 20）
  - `cd src-frontend && npm ci && npm run build`（前端能否构建）
  - `cd src-tauri && cargo check`（后端能否编译）
  - `grep devUrl src-tauri/tauri.conf.json`（devUrl 是否指向 5173）
- 易漂移项：`rust-toolchain.toml`、`src-frontend/package.json` 依赖版本、`.github/workflows/build.yml` 的 runner/密钥名、系统包列表。
- 最后核对：2026-07-07，v0.26.23。
