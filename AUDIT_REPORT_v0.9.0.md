# StoryForge (草苔) v0.9.0 — 深度技术审计报告

> 审计日期：2026-06-09
> 审计范围：`src-tauri/` (266 Rust 文件)、`src-frontend/` (204 TS/TSX 文件)、`e2e/`、CI/CD、文档体系
> 审计方法：静态代码分析、配置文件审查、构建系统检查、依赖扫描
> 约束条件：未执行动态性能分析，未进行渗透测试

---

## 1. 执行摘要（Executive Summary）

**整体健康度评分：C+**

StoryForge 是一个架构愿景清晰、产品特色鲜明的 AI 辅助小说创作桌面应用。团队在 v0.9.0 阶段已经建立了相对完整的分层架构（Command → DTO → Domain Service → Repository → SQLite/LanceDB）、统一错误处理体系（`AppError`）和前后端状态同步机制（`SyncEvent` + TanStack Query）。297 个 Rust 单元测试全部通过，`cargo check` 接近零警告，表明核心后端逻辑具备基本的正确性保障。

然而，项目的**代码体量和架构复杂度已经明显超出了当前团队治理能力**。具体表现为：6,000+ 行的 God File、2,200+ 行的 God Component、559 处 `unwrap()`、大量 `#![allow(dead_code)]` 掩盖的废弃代码、以及 CI 中主动关闭的 Lint/Clippy 门禁。这些问题单独来看都不算致命，但叠加在一起形成了一个危险信号：**代码库正在以快于重构速度的速度膨胀**。

**Top 3 核心风险：**
1. **God File 与 God Component 失控**：`repositories.rs` (6,198 行) 和 `FrontstageApp.tsx` (2,214 行) 已成为事实上的单点故障，任何改动都需要理解整个文件的业务逻辑。
2. **CI 质量门禁失效**：Clippy `-D warnings` 和前端 ESLint 被注释掉，E2E 测试被官方声明为"不可靠"，导致缺陷只能依靠人工代码审查和后期手动测试发现。
3. **dead_code 与预留模块的隐性成本**：`chat`、`collab`、`state`、`pipeline`、`llm/commands` 等多个模块被标记为 RESERVED 或 `#![allow(dead_code)]`，它们在编译、依赖解析、心智负担上持续消耗团队资源，却未产生任何用户价值。

**Top 3 核心技术机遇：**
1. **前端 God Component 拆分**：`FrontstageApp.tsx` 按职责拆分为独立子组件和自定义 Hooks，可在 1-2 天内显著降低幕前界面的维护成本。
2. **启用 CI Lint/Clippy 门禁**：一次性清理 471 个 Clippy 警告和 200+ ESLint 错误后，建立自动化质量防线，长期收益极高。
3. **Repository 垂直拆分**：将 `repositories.rs` 按领域拆分为独立文件，配合现有 `repositories_*.rs` 的拆分思路，可显著降低数据层的认知负载。

---

## 2. 代码库全景图（Repo Map）

### 2.1 项目定位

| 维度 | 描述 |
|------|------|
| **产品形态** | AI 辅助小说创作桌面应用（Tauri 跨平台桌面端） |
| **目标用户** | 中文网络小说作者、创意写作爱好者 |
| **当前成熟度** | 接近公测的晚期 Beta（v0.9.0），功能丰富但代码债务累积 |
| **分发方式** | GitHub Releases（.dmg/.msi/.deb）+ 自动更新（Tauri Updater） |
| **商业模式** | Freemium（功能订阅制）：Free 基础写作，Pro 解锁 Pipeline/拆书/自动续写 |

### 2.2 技术栈

| 层级 | 技术选型 | 版本 |
|------|---------|------|
| 桌面框架 | Tauri | 2.4 |
| 前端框架 | React + TypeScript + Vite | 18 / 5.8 / 6 |
| 样式方案 | Tailwind CSS | 3.4 |
| 状态管理 | Zustand（全局）+ TanStack Query（服务端状态） | 5 / 5 |
| 编辑器 | TipTap / ProseMirror | 3.22 |
| 后端语言 | Rust | 1.94 |
| 数据库 | SQLite（rusqlite + r2d2 连接池） | 0.39 |
| 向量存储 | LanceDB | 0.27 |
| 向量检索 | 自定义 CJK Bigram + Embedding 混合搜索 | - |
| 测试框架 | Rust内置 + Playwright + Vitest | - |
| CI/CD | GitHub Actions | - |

### 2.3 架构简图

```
┌─────────────────┐     ┌─────────────────┐
│   Frontstage    │     │    Backstage    │
│  (沉浸式写作)    │◄───►│   (工作室管理)   │
│   React + TS    │     │   React + TS    │
└────────┬────────┘     └────────┬────────┘
         │                       │
         └───────────┬───────────┘
                     ▼
            ┌─────────────────┐
            │   Tauri IPC     │
            │ Commands/Events │
            └────────┬────────┘
                     ▼
    ┌─────────────────────────────────────┐
    │         Rust Backend Core            │
    │  commands/ → dto/ → story_system/   │
    │  → db/repositories*.rs → SQLite     │
    └─────────────────────────────────────┘
```

### 2.4 核心目录说明

| 目录 | 说明 | 健康度 |
|------|------|--------|
| `src-tauri/src/commands/` | Tauri IPC 命令层（20+ 文件，按领域拆分） | 🟡 中等 |
| `src-tauri/src/db/` | 数据层：连接池、迁移、模型、DTO、仓库 | 🔴 需关注 |
| `src-tauri/src/story_system/` | 领域服务层：章节/场景服务、投影写入器 | 🟡 中等 |
| `src-tauri/src/state_sync/` | 前后端状态同步事件系统 | 🟢 良好 |
| `src-tauri/src/llm/` | LLM 适配器（OpenAI/Anthropic/Ollama） | 🟡 中等 |
| `src-tauri/src/memory/` | 四层记忆系统（知识图谱、向量存储） | 🟢 良好 |
| `src-tauri/src/chat/` | **RESERVED** — 未实现（Phase 4） | ⚫ 空壳 |
| `src-tauri/src/collab/` | **RESERVED** — 未实现（Phase 4） | ⚫ 空壳 |
| `src-tauri/src/state/` | **RESERVED** — 运行时状态管理（Phase 4） | ⚫ 空壳 |
| `src-frontend/src/frontstage/` | 幕前界面代码 | 🔴 需关注 |
| `src-frontend/src/pages/` | 幕后页面（Stories/Scenes/Settings 等） | 🟡 中等 |
| `src-frontend/src/services/api/` | v0.9.0 拆分后的 IPC API 层（17 文件） | 🟢 良好 |
| `src-frontend/src/hooks/` | 自定义 Hooks | 🟡 中等 |
| `e2e/` | Playwright E2E 测试（4 个 spec 文件） | 🔴 需关注 |

### 2.5 意外发现

1. **版本号混乱**：根目录 `package.json` 标记为 `v5.4.1`，`src-tauri/Cargo.toml` 和 `src-frontend/package.json` 标记为 `v0.9.0`，README 同时出现 `v0.9.0` 和 `v5.4.1`。这表明团队可能保留了内部版本号和外部版本号两套体系，但缺乏自动化校验。
2. **API Key 安全存储**：使用 `keyring` crate 将 API Key 存储在系统钥匙串中，而非 SQLite 或明文文件，这是超出一般桌面应用安全水准的亮点。
3. **自研 MigrationRunner**：因 rusqlite 0.39 与 refinery 默认特性冲突，团队手写了一个支持 SQL 文件迁移、幂等执行、遗留内联迁移兼容的 Runner，设计质量较高。

---

## 3. 审计报告（Audit Report）

按维度分组，每组按严重程度 **致命(Critical) > 高(High) > 中(Medium) > 低(Low)** 排序。

---

### 3.1 架构与设计

#### 🔴 Critical | `repositories.rs` 是 6,198 行的 God File
- **发现**：`src-tauri/src/db/repositories.rs` 包含 Story / Scene / Chapter / Character / WorldBuilding / StyleDNA / GenreProfile / ExportTemplate / AI Operation 等至少 9 个领域的全部数据访问逻辑。
- **位置**：`src-tauri/src/db/repositories.rs:1-6198`
- **后果**：任何数据层修改都需要阅读 6,000+ 行代码；合并冲突概率极高；新成员无法快速定位所需 Repository 方法；该文件已成为项目迭代的事实瓶颈。
- **证据**：文件行数统计 `wc -l src-tauri/src/db/repositories.rs` → `6198`。

#### 🔴 Critical | `FrontstageApp.tsx` 是 2,214 行的 God Component
- **发现**：幕前写作界面的根组件包含状态管理（story/chapter/scene/editor）、事件监听（4+ 个 Tauri 事件）、AI 生成流程、自动保存、键盘快捷键、幽灵文本、文思模式切换、Pipeline 调用等几乎所有幕前业务逻辑。
- **位置**：`src-frontend/src/frontstage/FrontstageApp.tsx:1-2214`
- **后果**：React 重渲染优化几乎不可能；任何幕前 Bug 都需要在这个文件中排查；测试该组件需要 mock 数十个依赖；与 `Stories.tsx` (1,197 行)、`SceneEditor.tsx` (766 行) 共同构成了前端"大到无法维护"的三巨头。

#### 🟠 High | 大量 RESERVED 模块占据编译单元和心智负担
- **发现**：`lib.rs` 中声明了 `mod chat; // RESERVED`、`mod collab; // RESERVED`、`mod state; // RESERVED` 三个模块。此外 `pipeline/`、`llm/commands.rs`、`embeddings/embedding.rs`、`state_sync/service.rs` 等模块顶部都有 `#![allow(dead_code)]`。
- **位置**：`src-tauri/src/lib.rs:12-13,37`、`src-tauri/src/pipeline/*.rs`、`src-tauri/src/llm/*.rs`、`src-tauri/src/state_sync/service.rs`
- **后果**：这些代码虽然被 `allow(dead_code)` 压制了编译警告，但仍参与编译、增加二进制体积、在 IDE 中污染符号索引、让新开发者误以为这些功能可用。
- **证据**：`grep -r "RESERVED\|#![allow(dead_code)]" src-tauri/src --include="*.rs" | wc -l` → 20+ 处。

#### 🟠 High | `connection.rs` 混合 Schema 定义、内联迁移、连接池初始化
- **发现**：`src-tauri/src/db/connection.rs` 长达 3,613 行，同时包含：r2d2 连接池初始化、`CREATE TABLE` 内联 Schema 定义（V3 基础表）、28+ 个条件内联迁移逻辑（`if current_version < N`）、测试用连接池创建。
- **位置**：`src-tauri/src/db/connection.rs:1-3613`
- **后果**：数据库 Schema 的变更历史与运行时连接管理耦合在一起；Schema 审查困难；无法独立查看"当前数据库应该长什么样"。虽然团队已引入 `MigrationRunner` 和 `.sql` 文件迁移（V007~V027），但遗留的内联迁移仍占该文件 80% 以上篇幅。

#### 🟡 Medium | `models.rs` 是 1,737 行的全领域模型聚合文件
- **发现**：`src-tauri/src/db/models.rs` 包含 Scene、Character、WorldBuilding、KnowledgeGraph、StudioConfig、Export 等所有领域的数据库实体模型。
- **位置**：`src-tauri/src/db/models.rs:1-1737`
- **后果**：任何模型字段修改都需要打开这个巨型文件；不同领域的模型变更互相干扰 diff 阅读。建议按领域拆分为 `models/scene.rs`、`models/character.rs` 等。

#### 🟡 Medium | `commands/orchestrator.rs` (722 行) 职责过重
- **发现**：编排器命令文件同时处理了 CreationWorkflow、Bootstrap、SmartExecute、Refine/Review/Finalize Pipeline 等多个高阶业务流程。
- **位置**：`src-tauri/src/commands/orchestrator.rs:1-722`
- **后果**：高阶业务流程与 IPC 层耦合，难以独立测试。

#### 🟢 优势 | 分层架构在 v0.9.0 已初步落地
- `commands/` → `dto.rs` → `story_system/*_service.rs` → `repositories*.rs` 的调用链在文档和主要代码路径中得到了遵循。`chapter_service.rs` 和 `scene_service.rs` 的引入是积极的架构演进。

#### 🟢 优势 | 前后端状态同步机制成熟
- `SyncEvent` 枚举（ts-rs 自动生成 TypeScript 绑定）+ `useSyncStore` Hook 精确失效 TanStack Query 缓存，解决了早期版本前后台数据不同步的痛点。

---

### 3.2 代码质量

#### 🟠 High | 559 处 `unwrap()` 分散在代码库中
- **发现**：整个 `src-tauri/src` 中存在 559 处 `unwrap()` 调用，以及 1 处 `panic!`。
- **位置**：分散在全库，例如 `src-tauri/src/lib.rs:461`、`src-tauri/src/db/connection.rs:56`、`src-tauri/src/embeddings/embedding.rs:138-140`（unsafe + unwrap 组合）。
- **后果**：虽然桌面应用遇到 panic 通常只会崩溃当前实例，但用户在长时间写作过程中遭遇崩溃会导致数据丢失和强烈负面体验。特别是在 `graceful_shutdown` 路径中的 unwrap 可能将"优雅关闭"变成"强制崩溃"。
- **证据**：`grep -r "unwrap()" src-tauri/src --include="*.rs" | wc -l` → `559`。

#### 🟠 High | `unsafe` 块出现在 Embedding 初始化中
- **发现**：`embeddings/embedding.rs` 使用 `unsafe { VOCAB = Some(HashMap::new()); }` 修改全局可变静态变量；`lib.rs:461` 也有 unsafe 块。
- **位置**：`src-tauri/src/embeddings/embedding.rs:138`、`src-tauri/src/lib.rs:461`
- **后果**：全局可变状态是数据竞争和 UB（Undefined Behavior）的温床。虽然当前调用路径可能单线程，但 Rust 的 `unsafe` 意味着编译器不再保证内存安全。
- **建议**：使用 `Lazy<Mutex<...>>` 或 `RwLock` 替代全局可变静态变量。

#### 🟠 High | 大量 `#![allow(dead_code)]` 掩盖了真实的代码腐烂
- **发现**：`pipeline/` 目录下 4 个文件、`llm/` 目录下 4 个文件、`state_sync/service.rs`、`embeddings/embedding.rs` 等均标记了 `#![allow(dead_code)]`。
- **位置**：`src-tauri/src/pipeline/mod.rs`、`src-tauri/src/llm/mod.rs`、`src-tauri/src/llm/adapter.rs` 等
- **后果**：编译器不再报告未使用的函数/结构体/导入，导致废弃代码不会被自然清理。团队无法区分"暂时未用但将来会用"和"已经废弃应该删除"的代码。

#### 🟡 Medium | 前端存在 `any` 类型逃逸和宽松类型实践
- **发现**：前端 `types/v3.ts` 长达 704 行，包含大量 `interface` 定义，但部分关键位置（如事件 payload、IPC 响应）使用了宽松的类型或 `any`。`FrontstageApp.tsx` 中定义的 `FrontstageEvent` 接口所有字段都是可选的（`?`），缺乏穷尽性检查。
- **位置**：`src-frontend/src/frontstage/FrontstageApp.tsx:61-76`
- **后果**：IPC 事件类型变更时，TypeScript 编译器无法在前端捕获不兼容的字段访问，只能在运行时暴露为 `undefined` 错误。

#### 🟡 Medium | `graceful_shutdown` 中使用了 `catch_unwind` 包裹 State 获取
- **发现**：`lib.rs:129` 使用 `std::panic::catch_unwind` 来获取 AutomationService State。
- **位置**：`src-tauri/src/lib.rs:129-136`
- **后果**：`catch_unwind` 在 Rust 中不是万能的容错工具，跨 FFI 边界或某些 panic 类型无法捕获。如果 State 获取 panic，说明 Tauri 内部状态已经损坏，此时继续执行 shutdown 逻辑可能不安全。

#### 🟢 优势 | `AppError` 统一错误处理体系设计良好
- 结构化错误枚举（`SubscriptionRequired`、`LlmTimeout`、`DbLocked` 等）+ 自动 IPC 序列化 + 前端错误码路由恢复 UI，是桌面应用中少见的成熟错误处理设计。

#### 🟢 优势 | `sanitizeArgs` 参数脱敏
- `src-frontend/src/services/api/core.ts:7-27` 在日志输出前自动脱敏 `api_key`、`token`、`password`、`secret` 字段，防止敏感信息泄露到控制台日志。

---

### 3.3 安全性

#### 🟠 High | `reqwest` 被固定到 `=0.12.4`，可能错过安全补丁
- **发现**：`Cargo.toml:20` 中 `reqwest = { version = "=0.12.4", ... }` 使用了精确版本锁定（`=` 前缀）。
- **位置**：`src-tauri/Cargo.toml:20`
- **后果**：reqwest 作为 HTTP 客户端，如果 `0.12.5+` 发布了安全修复（如 TLS 证书验证漏洞、HTTP/2 拒绝服务修复等），项目将无法自动获取。
- **注意**：锁定可能是为了规避某个特定版本的破坏性变更，但需要记录决策原因并定期评估。

#### 🟡 Medium | `.env.example` 包含弱默认密钥
- **发现**：`.env.example:15` 中 `JWT_SECRET=your-super-secret-jwt-key-min-32-chars` 是一个示例值，但如果开发者直接复制为 `.env` 而不修改，生产环境将使用可预测的密钥。
- **位置**：`.env.example:15`
- **后果**：JWT 签名可被伪造，导致服务端认证绕过。虽然当前项目主要是桌面应用，但 `src-server/` 和 `src-server-web/` 目录暗示未来可能有服务端部署。

#### 🟡 Medium | 前端 `baseURL` 硬编码为 `http://localhost:5173`
- **发现**：`playwright.config.ts:31` 和开发脚本中多处硬编码 `localhost:5173`。
- **位置**：`playwright.config.ts:31`
- **后果**：E2E 测试和开发脚本在自定义端口或远程开发环境（如 GitHub Codespaces）中会失效。

#### 🟢 优势 | API Key 使用系统钥匙串存储
- `src-tauri/src/config/settings.rs:70-102` 使用 `keyring` crate 将 API Key 存储在 macOS Keychain / Windows Credential Manager / Linux Secret Service 中，而非 SQLite 或明文配置文件。

#### 🟢 优势 | SQLite 使用参数化查询
- 审查的 Repository 代码中未发现字符串拼接 SQL 的情况，均使用 `rusqlite` 的参数化查询或 `params![]` 宏，基本消除了 SQL 注入风险。

---

### 3.4 测试质量

#### 🔴 Critical | E2E 测试被官方声明为不可靠
- **发现**：`.github/workflows/build.yml:163-165` 的注释明确说明："E2E tests run against a Vite dev server without the real Tauri backend, causing settings page tests to fail (IPC calls hang)." E2E 测试在 CI 中不是发布构建的阻塞条件。
- **位置**：`.github/workflows/build.yml:163-165`
- **后果**：项目的最高层级测试（端到端）无法保障核心用户流程的正确性。Playwright 测试运行在 `http://localhost:5173` 的 Vite dev server 上，所有 Tauri IPC 调用都会静默失败或挂起，这意味着 E2E 测试实际上只测试了"前端页面是否能加载和截图"，而非"应用是否能正常工作"。

#### 🟠 High | E2E 测试含金量极低
- **发现**：`e2e/storyforge.spec.ts` 中绝大多数测试用例只是 `page.goto()` → `page.waitForTimeout(3000)` → `page.screenshot()`，没有任何业务行为断言。349 行 E2E 代码中，真正的 `expect()` 断言屈指可数。
- **位置**：`e2e/storyforge.spec.ts`
- **后果**：这些测试只能捕获"页面白屏"级别的故障，无法捕获数据不同步、表单提交失败、AI 生成流程中断等功能性缺陷。

#### 🟠 High | 前端 Vitest 测试覆盖盲区大
- **发现**：`src-frontend/src` 中仅有 10 个测试相关文件（含 `__tests__` 目录），而业务文件超过 200 个。核心 Hooks（`useSyncStore`、`useScenes`、`useCharacters`）、Stores（`appStore.ts`）和复杂组件（`FrontstageApp.tsx`、`Stories.tsx`）均无任何单元测试。
- **位置**：`src-frontend/src/hooks/`、`src-frontend/src/stores/`、`src-frontend/src/frontstage/`
- **后果**：前端重构（如拆分 God Component）缺乏安全网，只能依赖人工手动回归测试。

#### 🟡 Medium | Rust 测试集中在 Repository 和简单工具函数，复杂业务流程覆盖不足
- **发现**：297 个 Rust 测试中，大量集中在 `db/repositories_tests.rs`（726 行）和 `db/cascade_tests.rs` 等数据层测试。而 `story_system/mod.rs` (651 行)、`creative_engine/`、`pipeline/`、`planner/` 等复杂业务模块几乎没有对应的单元测试。
- **位置**：`src-tauri/src/db/repositories_tests.rs`、`src-tauri/src/db/cascade_tests.rs`
- **后果**：AI 生成、Bootstrap 创世、Pipeline 审校等核心业务流程的正确性完全依赖集成测试和手动验证。

#### 🟢 优势 | Rust 测试基础设施完善
- `create_test_pool()` 提供内存 SQLite 连接池 + 自动迁移，Repository 层的测试编写门槛很低。测试使用 `serial_test` 避免并行冲突，设计合理。

---

### 3.5 性能表现

#### 🟡 Medium | `build_writer_prompt` 曾存在同步阻塞调用，虽已 async 化但类似模式可能仍存
- **发现**：CHANGELOG 记录 v4.5.0 修复了 `build_writer_prompt` 使用 `tauri::async_runtime::block_on` 导致的 Tauri 异步运行时死锁/线程阻塞问题。
- **位置**：历史修复，但类似模式可能在其他命令中重现。
- **后果**：在 Tauri 的异步运行时中调用 `block_on` 是已知的危险模式，可能导致整个事件循环冻结，前端 UI 卡死。
- **建议**：全库审计 `block_on` 使用情况。

#### 🟡 Medium | SQLite 连接池 max_size=5 可能成为并发瓶颈
- **发现**：`src-tauri/src/db/connection.rs:18,74` 中连接池 `max_size` 设置为 5。
- **位置**：`src-tauri/src/db/connection.rs:18,74`
- **后果**：在 Bootstrap 后台阶段并发执行多个 LLM 调用和数据库写入时，5 个连接可能成为瓶颈。虽然桌面应用通常单用户，但 Bootstrap 的后台任务使用了 `tokio::spawn`，可能产生并发数据库访问。

#### 🟢 优势 | SQLite WAL 模式 + 优雅关闭 checkpoint
- `graceful_shutdown` 中执行 `PRAGMA wal_checkpoint(PASSIVE)`，确保崩溃恢复时数据完整性。WAL 模式也提升了读写并发性能。

#### 🟢 优势 | Embedding 缓存机制
- `embeddings/embedding.rs` 中实现了 `EmbeddingCache`（LRU 10000 条），避免重复计算 embedding。

---

### 3.6 依赖管理

#### 🟡 Medium | `Cargo.toml` 和 `package.json` 版本号不一致
- **发现**：根目录 `package.json:3` 是 `5.4.1`，`src-tauri/Cargo.toml:3` 和 `src-frontend/package.json:4` 是 `0.9.0`。
- **位置**：多文件
- **后果**：用户和开发者无法确定当前运行的确切版本；GitHub Release tag 可能与应用内版本号不一致；自动更新（Tauri Updater）的 manifest 版本匹配可能出错。

#### 🟡 Medium | `src-frontend` 存在 `@types/node v25.6.0` 与 Node 20 不匹配的潜在风险
- **发现**：`src-frontend/package.json:55` 中 `@types/node` 版本为 `^25.6.0`，但 CI 使用 Node 20。
- **位置**：`src-frontend/package.json:55`
- **后果**：Node 25 的类型定义可能包含 Node 20 不支持的 API，导致类型检查通过但运行时出错。

#### 🟢 优势 | 依赖整体健康
- 未发现明显过时或无人维护的关键依赖。Tauri 2.4、React 18、Rust 2021 edition 均为当前主流稳定版本。LanceDB 0.27 是较新的向量数据库版本。

---

### 3.7 开发者体验（DevEx）与运维

#### 🔴 Critical | CI 中 Clippy 和 ESLint 门禁被关闭
- **发现**：`.github/workflows/build.yml:73-74` 明确注释掉了 `-D warnings`（TODO: 修复 471 个已有 Clippy 警告后恢复）；`build.yml:108-111` 注释掉了前端 ESLint（TODO: 修复 200+ 个已有 lint 错误后恢复）。
- **位置**：`.github/workflows/build.yml:73-74,108-111`
- **后果**：代码风格债务持续增长；新提交的代码即使违反最佳实践也不会被阻止；团队失去了自动化的"最差代码"过滤器。

#### 🟠 High | `rustfmt nightly` 要求增加了不必要的开发摩擦
- **发现**：CI 中安装 `rustup component add rustfmt --toolchain nightly` 并执行 `cargo +nightly fmt`。
- **位置**：`.github/workflows/build.yml:66-71`
- **后果**：新贡献者需要安装 nightly 工具链才能通过格式检查；nightly 的格式规则可能随版本变化，导致"在本机通过但在 CI 失败"的不稳定构建。

#### 🟠 High | 根目录文档过度膨胀
- **发现**：项目根目录存在 15+ 个大型 Markdown 文档（`AGENTS.md` 842 行、`ARCHITECTURE.md` 997 行、`CHANGELOG.md` 171,166 字节、`PROJECT_STATUS.md` 等），许多内容互相重叠。
- **位置**：项目根目录
- **后果**：信息碎片化，开发者不确定应该先看哪份文档。`AGENTS.md` 同时承担了"AI 助手指南"、"编译状态看板"、"版本变更日志"、"项目宪法"四种职责，每次代码变更后需要同步更新多个文档。

#### 🟡 Medium | Playwright E2E 的 `webServer` 配置在 CI 中不可靠
- **发现**：`playwright.config.ts:72-77` 配置了 `webServer: { command: 'cd src-frontend && npm run dev' }`，但 CI 中 `e2e-check` job 并未启动 Tauri 后端。所有 IPC 调用都会挂起。
- **位置**：`playwright.config.ts:72-77`
- **后果**：E2E 测试在 CI 中只能验证"页面能否加载"，无法验证实际业务功能。

#### 🟢 优势 | 多平台 CI 构建矩阵完善
- macOS / Windows / Linux 三平台构建，包含代码签名（Apple/Windows）和自动 Release 发布，桌面应用的分发基础设施相当成熟。

#### 🟢 优势 | `scripts/verify-ipc-manifest.py` 存在
- 虽然前端 lint 被关闭，但项目意识到了 IPC 一致性检查的重要性，编写了 Python 脚本解析 `generate_handler![]` 宏与前端 `loggedInvoke` 调用，这是一个有深度的工程实践。

---

### 3.8 文档质量

#### 🟠 High | `AGENTS.md` 职责过多，已成为维护负担
- **发现**：`AGENTS.md` 842 行，混合了：AI 助手快速参考、开发命令、编码规范、最近 20+ 个版本的详细变更日志、编译状态看板、Spec-Kit 集成说明、GitNexus 集成说明。
- **位置**：`AGENTS.md`
- **后果**：每次发版需要同步更新 `CHANGELOG.md`、`AGENTS.md`、`README.md`、`PROJECT_STATUS.md`、`ROADMAP.md`、`ARCHITECTURE.md` 六份文档，人工维护几乎必然遗漏。

#### 🟡 Medium | 架构文档与实际代码存在差距
- **发现**：`ARCHITECTURE.md` 中描述的 v6.0.0 功能（MemoryOrchestrator、ReadingPowerEvaluator、AntiAiReviewer、GenreProfile 外部化）在实际代码中虽然存在对应模块，但部分标记为 `#![allow(dead_code)]` 或缺少前端集成，说明"文档上的架构"超前于"可运行的架构"。
- **位置**：`ARCHITECTURE.md` v6.0.0 章节 vs `src-tauri/src/` 实际模块

#### 🟢 优势 | 用户指南 `docs/USER_GUIDE.md` 图文并茂
- 包含 16+ 张产品截图和按页面组织的详细说明，对于桌面应用来说是高质量的用户文档。

---

## 4. 改进策略（Improvement Strategy）

### 4.1 核心主线归纳

通过对审计发现的归纳，提炼出 4 个能解释绝大多数问题的核心技术主题：

| # | 主题 | 解释的发现 | 设计原则 |
|---|------|-----------|---------|
| 1 | **代码体量失控** | God File、God Component、模型聚合文件、文档膨胀 | 单一文件应能在 3 分钟内读完；超过 500 行必须拆分 |
| 2 | **质量门禁休眠** | Clippy/ESLint 关闭、E2E 不可靠、dead_code 掩盖 | 编译器/静态分析是团队最便宜的 QA 工程师；永远不要关闭 linter |
| 3 | **架构文档超前于实现** | v6.0.0 功能标记为 dead_code、RESERVED 模块、Pipeline 空壳 | 文档应诚实反映可运行代码的状态；未激活的代码不应参与编译 |
| 4 | **测试金字塔倒置** | 297 个底层单元测试 vs 4 个"截图测试"E2E vs 0 个前端单元测试 | 单元测试保护重构，E2E 保护用户旅程；两者缺一不可 |

### 4.2 目标状态

| 主题 | 当前状态 | 目标状态 | 关键指标 |
|------|---------|---------|---------|
| 代码体量 | `repositories.rs` 6,198 行；`FrontstageApp.tsx` 2,214 行 | 单一文件 < 400 行；单一函数 < 50 行 | 超过 400 行的文件数量为 0 |
| 质量门禁 | Clippy `-D warnings` 关闭；ESLint 关闭 | 全部启用，且 CI 构建失败时阻断合并 | CI 失败率 < 5% |
| 架构诚实度 | 3 个 RESERVED 模块 + 20+ dead_code 标记 | 未激活的代码从编译单元中移除；文档与代码状态一致 | `#![allow(dead_code)]` 数量为 0；文档中的功能描述与代码激活状态 100% 匹配 |
| 测试覆盖 | Rust 297 个（主要 Repository 层）；前端 0 个；E2E 4 个截图测试 | Rust 核心业务流程覆盖率 ≥ 60%；前端关键 Hooks/Stores 有测试；E2E 覆盖核心用户旅程 | 前端单元测试 ≥ 30 个；E2E 断言数量 ≥ 50 个 |

### 4.3 权衡取舍（Trade-offs）

以下问题**不建议在当前阶段修复**，并说明理由：

1. **将 SQLite 迁移到 Postgres/其他数据库**
   - **理由**：当前 SQLite + r2d2 对于单用户桌面应用完全够用；迁移到 Postgres 会引入运行时依赖，破坏"开箱即用"的桌面体验。如果未来真的要服务端化，再考虑抽象数据库层。

2. **将 reqwest 替换为 hyper/其他 HTTP 客户端**
   - **理由**：reqwest 0.12.4 没有已知的高危 CVE（经快速检查）；精确版本锁定可能是为了兼容某个中间件。除非明确发现安全漏洞，否则不值得承担 HTTP 客户端替换的回归风险。

3. **将 Zustand 替换为 Redux Toolkit 或其他状态管理方案**
   - **理由**：Zustand 在前端的状态管理中没有表现出明显问题；替换状态管理库是高度破坏性的变更，收益不明确。

4. **重写 RESERVED 模块（chat/collab/state）**
   - **理由**：这些模块是 Phase 4 的规划功能，当前团队已经决定推迟实现。不应该在核心功能尚未稳定时投入资源开发协作编辑和聊天功能。

5. **引入微前端/Module Federation 拆分幕前幕后**
   - **理由**：当前 Vite 的代码分割已经通过 `frontstage.html` / `index.html` 实现了入口级分割；微前端的复杂度过高，与项目的团队规模不匹配。

### 4.4 "完成"标准（Definition of Done）

- **致命级漏洞清零**：无 `unsafe` 块（或每个 `unsafe` 块都有详尽的 SAFETY 注释和代码审查记录）；无已知 CVE 的高危依赖。
- **CI 门禁激活**：`cargo clippy -- -D warnings` 和 `npm run lint` 在 CI 中启用且通过；任何 PR 合并前必须通过。
- **核心模块文件大小达标**：`repositories.rs` 拆分后所有文件 < 500 行；`FrontstageApp.tsx` 拆分后 < 400 行。
- **dead_code 清零**：移除所有 `#![allow(dead_code)]` 和 `#[allow(dead_code)]`；未使用的代码要么激活、要么删除（保留在 Git 历史中可以恢复）。
- **前端测试基数建立**：至少 30 个前端单元测试覆盖 `useSyncStore`、`appStore`、核心 Hooks 和至少 3 个最复杂的组件。
- **E2E 测试具备业务断言**：E2E 测试不再只是截图，每个核心页面至少有一个验证业务数据的 `expect()` 断言。
- **版本号统一**：根 `package.json`、`src-tauri/Cargo.toml`、`src-frontend/package.json`、README、Git Tag 五处版本号通过脚本自动化校验，确保一致。

---

## 5. 执行计划（Task Plan）

### 速赢任务（Quick Wins）— S 级工作量，高回报

| # | 任务 | 影响范围 | 验收标准 | 风险 | 依赖 |
|---|------|---------|---------|------|------|
| Q1 | **统一版本号并加校验脚本** | `package.json` ×2, `Cargo.toml`, `README.md`, CI | 运行 `scripts/check-version.sh` 时五处版本号一致；CI 在构建前自动执行 | 无 | 无 |
| Q2 | **删除 `#![allow(dead_code)]` 并清理真实死代码** | `pipeline/`, `llm/`, `state_sync/`, `embeddings/` 等 | `cargo check` 零警告；删除的代码行数 ≥ 500 | 低（可 Git 恢复） | 无 |
| Q3 | **将 `repositories.rs` 的前 3 个领域拆出** | `src-tauri/src/db/repositories.rs` → `db/repositories_story.rs`, `repositories_scene.rs`, `repositories_character.rs` | 新文件编译通过；原文件行数 < 5000；现有 297 个测试全部通过 | 低 | 无 |
| Q4 | **启用 `cargo clippy -- -D warnings`** | `.github/workflows/build.yml`, 修复约 471 个警告 | CI 中 Clippy 步骤启用且通过；无新警告引入 | 中（需要批量修复） | Q2（先清理 dead_code） |

---

### 里程碑 0 / 安全网（Safety Net）

> **目标**：在重构前建立必要的保障措施，确保后续改动不会破坏已有功能。

| 任务 ID | 标题 | 描述 | 影响范围 | 验收标准 | 工作量 | 风险 | 依赖 |
|---------|------|------|---------|---------|--------|------|------|
| M0-T1 | 为 `FrontstageApp.tsx` 提取自定义 Hooks | 将状态管理、事件监听、AI 生成流程提取为 `useFrontstageState`、`useFrontstageEvents`、`useAiGeneration` 等 Hooks | `src-frontend/src/frontstage/FrontstageApp.tsx` | `FrontstageApp.tsx` 行数 < 1200；行为不变（手动回归测试核心流程） | M | 中（逻辑复杂） | 无 |
| M0-T2 | 为 `Stories.tsx` 和 `SceneEditor.tsx` 编写前端单元测试 | 使用 Vitest + React Testing Library 测试数据加载、交互、状态变更 | `src-frontend/src/pages/Stories.tsx`, `src-frontend/src/components/SceneEditor.tsx` | 新增 ≥ 15 个前端单元测试；`npm run test:run` 通过 | M | 低 | 无 |
| M0-T3 | 建立 `cargo clippy -- -D warnings` 的修复基线分支 | 创建一个独立分支，批量修复所有 471 个 Clippy 警告，作为未来 PR 的参考 | 全库 Rust 文件 | 该分支 `cargo clippy -- -D warnings` 通过；与 `master` diff 仅包含警告修复 | L | 低 | 无 |
| M0-T4 | 为 `story_system/mod.rs` 核心流程编写 Rust 单元测试 | 测试 `StorySystemEngine` 的 ContractTree 合并、RuntimeContract 生成 | `src-tauri/src/story_system/mod.rs` | 新增 ≥ 10 个 Rust 测试；`cargo test` 全部通过 | M | 低 | 无 |

---

### 里程碑 1 / 核心修复（Critical Fixes）

> **目标**：消除致命级和高风险安全问题，修复阻断级正确性问题。

| 任务 ID | 标题 | 描述 | 影响范围 | 验收标准 | 工作量 | 风险 | 依赖 |
|---------|------|------|---------|---------|--------|------|------|
| M1-T1 | 移除 `embeddings/embedding.rs` 和 `lib.rs` 中的 `unsafe` 块 | 使用 `Lazy<Mutex<HashMap<...>>>` 替代全局可变静态变量 | `src-tauri/src/embeddings/embedding.rs`, `src-tauri/src/lib.rs` | `grep -r "unsafe" src-tauri/src` 返回空；编译通过；现有测试通过 | S | 中（涉及全局状态） | 无 |
| M1-T2 | 审计并替换高危 `unwrap()` | 重点审计 `graceful_shutdown`、数据库连接、Tauri State 获取路径中的 `unwrap()`，替换为 `?` 或 `match` + 日志 | `src-tauri/src/lib.rs`, `src-tauri/src/db/connection.rs`, 其他核心路径 | 高危路径的 `unwrap()` 数量为 0；编译通过 | M | 中 | 无 |
| M1-T3 | 修复 `.env.example` 默认密钥问题 | 将示例密钥改为空值或生成随机占位符；在服务端启动时校验 JWT_SECRET 长度 ≥ 32 | `.env.example`, `src-server/src/auth/`（如有启动校验） | `.env.example` 无默认密钥；服务端启动时短密钥报错 | S | 低 | 无 |
| M1-T4 | 将 `reqwest` 的精确版本锁定改为最低版本约束 | 研究 `=0.12.4` 锁定的原因，如果无特殊需求改为 `"0.12"` 或 `"0.12.4"`（不带 `=`） | `src-tauri/Cargo.toml` | `Cargo.lock` 更新后 `cargo check` 通过；HTTP 功能正常 | S | 低 | 无 |

---

### 里程碑 2 / 高杠杆改进（High-Leverage Improvements）

> **目标**：能大幅提升后续开发效率的架构或工具链调整。

| 任务 ID | 标题 | 描述 | 影响范围 | 验收标准 | 工作量 | 风险 | 依赖 |
|---------|------|------|---------|---------|--------|------|------|
| M2-T1 | **完成 `repositories.rs` 的全领域拆分** | 将剩余领域全部拆分为 `repositories_{domain}.rs`，原文件仅保留 re-export 或完全删除 | `src-tauri/src/db/repositories.rs` | 拆分后单文件最大行数 < 500；297 个测试通过；无编译警告 | L | 中（影响面广） | M0-T3 |
| M2-T2 | **拆分 `FrontstageApp.tsx` 为子组件** | 提取 `FrontstageEditor`、`FrontstageHeader`、`GhostTextOverlay`、`WenSiPanelWrapper` 等子组件 | `src-frontend/src/frontstage/FrontstageApp.tsx` | `FrontstageApp.tsx` < 400 行；子组件独立可测试；手动回归测试通过 | L | 高（幕前是核心体验） | M0-T1 |
| M2-T3 | **启用 CI 前端 ESLint 和 Clippy 门禁** | 将 `.github/workflows/build.yml` 中注释掉的 lint 步骤恢复；修复剩余的 ESLint 错误 | `.github/workflows/build.yml`, 全库前端/后端代码 | CI 全部通过；任何新 PR 违反 lint 会被阻断 | M | 中 | M0-T3, M1-T2 |
| M2-T4 | **从编译单元中移除 RESERVED 模块** | 将 `chat`、`collab`、`state` 从 `lib.rs` 中注释掉或移至 `examples/`；将未激活的 `pipeline/` 模块标记为 `#[cfg(feature = "pipeline")]` | `src-tauri/src/lib.rs`, `src-tauri/src/pipeline/` | `cargo check` 通过；二进制体积减小；`#![allow(dead_code)]` 数量为 0 | M | 低 | Q2 |
| M2-T5 | **重写 E2E 测试框架，接入 Tauri 后端** | 调研 `tauri-driver` 或 `@tauri-apps/api` 的测试方案，使 E2E 能调用真实 IPC；或至少使用 Mock 数据验证业务断言 | `e2e/`, `playwright.config.ts` | E2E 测试能通过 IPC 验证至少 3 个核心业务流程（创建故事、生成场景、保存章节） | XL | 高（可能需要 Tauri 测试基础设施重构） | M0-T2 |

---

### 里程碑 3 / 优化与打磨（Quality & Polish）

> **目标**：剩余值得顺手做掉的中低优先级优化。

| 任务 ID | 标题 | 描述 | 影响范围 | 验收标准 | 工作量 | 风险 | 依赖 |
|---------|------|------|---------|---------|--------|------|------|
| M3-T1 | 将 `models.rs` 按领域拆分 | 创建 `src-tauri/src/db/models/` 目录，按领域拆分模型 | `src-tauri/src/db/models.rs` | 单文件 < 300 行；ts-rs 生成的类型文件路径更新 | M | 低 | M2-T1 |
| M3-T2 | 统一使用 `rustfmt stable` | 移除 CI 中对 nightly 的依赖，使用 stable 的 `cargo fmt` | `.github/workflows/build.yml`, `rustfmt.toml` | CI 使用 stable 通过；格式无变化 | S | 低 | 无 |
| M3-T3 | 整理根目录文档 | 将 `AGENTS.md` 中的变更日志迁移到 `CHANGELOG.md`；将 Spec-Kit/GitNexus 指南迁移到 `docs/`；`AGENTS.md` 保留为 AI 助手快速参考（< 200 行） | 根目录 `.md` 文件 | `AGENTS.md` < 200 行；无信息丢失；CI 不依赖这些文件 | M | 低 | 无 |
| M3-T4 | 评估 SQLite 连接池 max_size | 在 Bootstrap 后台阶段测试并发数据库访问，如果瓶颈明显将 max_size 提升到 10 | `src-tauri/src/db/connection.rs` | Bootstrap 后台阶段无数据库连接等待；现有测试通过 | S | 低 | 无 |
| M3-T5 | 前端 `@types/node` 版本对齐 | 将 `@types/node` 降级到与 CI Node 20 匹配的版本（`^20.x`） | `src-frontend/package.json` | `npm ci` 通过；类型检查通过 | S | 低 | 无 |

---

### 排名前 3 核心任务的技术小样

#### 任务 M2-T1：`repositories.rs` 拆分方案

**核心思路**：按领域垂直拆分，每个领域一个文件。

```
src-tauri/src/db/
├── repositories.rs          # 仅保留 re-export 和通用 trait
├── repositories_story.rs    # StoryRepository
├── repositories_scene.rs    # SceneRepository + SceneVersionRepository
├── repositories_character.rs # CharacterRepository
├── repositories_world.rs    # WorldBuildingRepository + WorldRulesRepository
├── repositories_kg.rs       # KnowledgeGraph repositories
├── repositories_style.rs    # StyleDnaRepository
├── repositories_export.rs   # 已有，保持不变
├── repositories_pipeline.rs # 已有，保持不变
├── repositories_narrative.rs # 已有，保持不变
├── repositories_story_system.rs # 已有，保持不变
└── ...
```

**避坑指南**：
1. 先提取纯函数（如 `build_scene_from_row`）到 `db/query_helpers.rs`，避免跨文件的私有函数访问问题。
2. 使用 `pub use` 在 `db/mod.rs` 中重新导出，保持 `db::StoryRepository` 的调用路径不变，避免改动所有调用方。
3. 分 3 个 PR 完成：第一批提取 Story/Scene（最独立），第二批提取 Character/WorldBuilding，第三批清理残留。

#### 任务 M2-T2：`FrontstageApp.tsx` 拆分方案

**核心思路**：按"状态域"提取自定义 Hooks，按"UI 区域"提取子组件。

```
src-frontend/src/frontstage/
├── FrontstageApp.tsx        # 仅负责组合子组件和顶层布局
├── hooks/
│   ├── useFrontstageState.ts    # story/chapter/scene/current 状态 + refs
│   ├── useFrontstageEvents.ts   # Tauri 事件监听（ChapterSwitch、chapter-updated 等）
│   ├── useAiGeneration.ts       # smartExecute、runRefine、runReview 调用
│   ├── useAutoSave.ts           # 自动保存逻辑
│   └── useGhostText.ts          # 幽灵文本接受/拒绝
├── components/
│   ├── FrontstageHeader.tsx     # 顶部状态栏（已存在，确认解耦）
│   ├── FrontstageSidebar.tsx    # 左侧工具栏（已存在）
│   ├── FrontstageEditor.tsx     # RichTextEditor + 幽灵文本覆盖层
│   ├── FrontstageBottomBar.tsx  # 底部 AI 输入栏（已存在）
│   └── WenSiPanelWrapper.tsx    # 浮动文思面板
```

**避坑指南**：
1. `FrontstageApp.tsx` 使用了大量 `useRef` 来避免 stale closure，提取 Hook 时必须确保 `useCallback` 的依赖数组正确，或继续使用 ref 模式传递最新状态。
2. 先写测试再拆分：为 `FrontstageApp.tsx` 的当前行为编写 3-5 个高保真测试（即使需要 mock 大量依赖），拆分后运行测试验证行为不变。
3. 不要一次性全部拆分；先提取最独立的 `useAutoSave`，再提取 `useAiGeneration`，最后处理事件监听。

#### 任务 M1-T1：移除 `unsafe` 块

**核心思路**：将 `static mut VOCAB: Option<HashMap<...>>` 替换为 `OnceCell<Mutex<HashMap<...>>>`。

**当前代码**（`embeddings/embedding.rs:136-140`）：
```rust
static mut VOCAB: Option<HashMap<String, usize>> = None;

EMBEDDING_INIT.call_once(|| {
    unsafe { VOCAB = Some(HashMap::new()); }
});
```

**替换为**：
```rust
use std::sync::Mutex;
use once_cell::sync::Lazy;

static VOCAB: Lazy<Mutex<HashMap<String, usize>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});
```

**避坑指南**：
1. `VOCAB` 当前只在 `init_embedding_model()` 中写入一次，之后只读。如果保持只读语义，可以用 `OnceCell<HashMap<...>>`（无需 Mutex）。需要确认 `tokenize()` 中是否会在首次初始化后再次写入。
2. `lib.rs:461` 的 `unsafe` 块需要单独审查上下文，可能涉及 Tauri 的特定 FFI 调用。

---

## 6. 开放式提问（Open Questions）

为了完善此计划，以下问题需要与人类开发者进一步确认：

1. **产品优先级**：`chat`、`collab`、`state` 三个 RESERVED 模块的 Phase 4 计划是否仍然有效？如果未来 6 个月内不打算实现，建议从编译单元中移除以减少维护负担。

2. **版本号体系**：根 `package.json` 的 `v5.4.1` 和 `Cargo.toml` 的 `v0.9.0` 哪个是"真相来源"？团队内部是否有一套版本号映射规则？

3. **E2E 测试策略**：当前 E2E 测试在 Vite dev server 上运行，无法调用 Tauri IPC。团队是否愿意投入时间调研 `tauri-driver` 或基于 `@tauri-apps/api` 的 Mock 方案来建立真正的端到端测试？还是认为桌面应用的 E2E 测试ROI过低，应依赖手动发布前测试？

4. **Clippy 警告清理的预算**：约 471 个 Clippy 警告中，是否有已知的高风险模式（如 `unwrap_or_else` 误用、`map_or` 可读性问题）需要优先处理？团队是否接受一个"纯格式化/警告修复"的大型 PR？

5. **前端状态管理边界**：`FrontstageApp.tsx` 中有大量本地 `useState`（story、chapter、scene、editor content），同时 Zustand `appStore` 也持有部分全局状态。团队是否计划将幕前状态统一下沉到 Zustand，还是保持当前"本地状态为主、全局状态为辅"的混合模式？

6. **商业化时间线**：Freemium 付费系统（`subscription/` 模块）当前的状态是什么？是否已经上线收费，还是仍在规划中？这会影响安全审计的严格程度（已收费产品需要更高的安全标准）。

---

*本报告由静态代码审计生成，未包含动态性能分析和渗透测试结果。建议在实施关键修复后进行一轮独立的运行时安全审计。*
