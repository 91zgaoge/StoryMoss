# StoryMoss 项目架构与代码检视报告

---

## 1. 项目概况

**StoryMoss（草苔）** 是一个基于 Tauri 的 AI 辅助小说创作桌面应用，采用 monorepo 结构，包含两套后端和两套前端。Tauri 桌面端（`src-tauri` + `src-frontend`）是主要产品——一个本地优先的应用，使用 SQLite 做数据持久化，LLM API 做 AI 写作辅助，LanceDB 向量库做知识检索。服务端部署（`src-server` + `src-server-web`）提供 OAuth2/JWT 认证和 PostgreSQL 后端的多用户场景。

应用采用双窗口架构：幕前（写作界面）和幕后（故事管理）。前端通过 Tauri IPC 命令与 Rust 后端通信，而非 HTTP API。

**版本**: 0.8.0（Tauri 桌面端），4.5.0（服务端/Web 组件）

---

## 2. 架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                          CLIENT (Desktop)                           │
│                     Tauri 2.x + WebView                              │
├─────────────────────────────────────────────────────────────────────┤
│  ┌──────────────────┐    ┌──────────────────────────────────────┐   │
│  │   Frontstage     │    │          Backstage                   │   │
│  │  (幕前 - Writing) │    │      (幕后 - Management)             │   │
│  │                  │    │                                      │   │
│  │  TipTap Editor   │    │  ┌─────────┐ ┌────────┐ ┌─────────┐ │   │
│  │  Monaco Editor   │    │  │Stories │ │Chapters│ │Characters│ │   │
│  │  AI Streaming    │    │  │Scenes  │ │Skills  │ │Settings  │ │   │
│  │  Smart Hints     │    │  └─────────┘ └────────┘ └─────────┘ │   │
│  │                  │    │                                      │   │
│  │  FrontstageApp   │    │  App.tsx + Sidebar + View Router     │   │
│  │  (1800+ lines)   │    │                                      │   │
│  └────────┬─────────┘    └──────────────┬───────────────────────┘   │
│           │                             │                            │
│  ┌────────▼─────────────────────────────▼──────────────────────────┐│
│  │                    Tauri IPC Layer                              ││
│  │  loggedInvoke<T>() -- RPC wrapper with sanitization + timing    ││
│  │  ~1285-line file, 100+ command handlers                         ││
│  └──────────────────────────┬─────────────────────────────────────┘│
│                              │                                     │
│  ┌───────────────────────────▼───────────────────────────────────┐│
│  │                    Tauri Backend (Rust)                        ││
│  │  src-tauri/ -- Cargo workspace member                          ││
│  │                                                                  ││
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐  ││
│  │  │ Commands │ │ Creative │ │ Narrative│ │ Story System     │  ││
│  │  │ Layer    │ │ Engine   │ │ Layer    │ │ (Contracts)      │  ││
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────────────┘  ││
│  │                                                                  ││
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐  ││
│  │  │ LLM/     │ │ MCP      │ │ Task     │ │ Pipeline         │  ││
│  │  │ Embedding│ │ Protocol │ │ System   │ │ Orchestrator     │  ││
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────────────┘  ││
│  │                                                                  ││
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐  ││
│  │  │ Memory   │ │ Evolution│ │ Vector   │ │ Anti-AI          │  ││
│  │  │ System   │ │ Engine   │ │ Search   │ │ Review           │  ││
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────────────┘  ││
│  │                                                                  ││
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐  ││
│  │  │ Auth     │ │ Export   │ │ Skills   │ │ Knowledge Base   │  ││
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────────────┘  ││
│  └────────────────────────────────────────────────────────────────┘│
│                              │                                     │
│  ┌───────────────────────────▼───────────────────────────────────┐│
│  │                    Data Storage                               ││
│  │  ┌─────────────┐  ┌─────────────┐  ┌──────────────────────┐  ││
│  │  │ SQLite      │  │ LanceDB     │  │ OS Keychain          │  ││
│  │  │ 73 migrations│  │ Vector DB   │  │ API key storage      │  ││
│  │  │ ~50 tables  │  │             │  │                      │  ││
│  │  └─────────────┘  └─────────────┘  └──────────────────────┘  ││
│  └────────────────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────────────┤
│                          SERVER (Web Deployment)                    │
│                     actix-web + PostgreSQL                          │
│                                                                      │
│  ┌──────────────────┐    ┌──────────────────────────────────────┐   │
│  │  src-server/     │    │  src-server-web/                      │   │
│  │  (Rust backend)  │    │  (Nginx + React frontend)             │   │
│  │                  │    │                                      │   │
│  │  /api/health     │    │  SPA fallback                         │   │
│  │  /api/auth/*     │    │  /api/ -> proxy to backend:8080      │   │
│  │  OAuth2/JWT      │    │                                      │   │
│  └──────────────────┘    └──────────────────────────────────────┘   │
│                              │                                     │
│                      PostgreSQL 16                                 │
│                      3 tables (users, oauth_accounts, sessions)     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. 技术栈摘要

| 层级 | 技术 | 版本 | 用途 |
|------|------|------|------|
| **桌面壳层** | Tauri 2.x | 2.4 | Native app with WebView windows |
| **Rust 后端 (Tauri)** | Cargo workspace | 2021 edition | IPC commands, business logic, SQLite, LLM integration |
| **Rust 服务端** | actix-web | 4 | HTTP API for web deployment |
| **前端框架** | React | 18.3 | UI components |
| **构建工具** | Vite | 6.2.5 | Dev server + bundler |
| **TypeScript** | tsc | 5.8.3 | Type checking and compilation |
| **状态管理** | Zustand | 5.0.3 | Global state (5 stores) |
| **数据获取** | TanStack Query | 5.71 | Cache + async data fetching |
| **路由** | Manual switch | -- | Zustand-backed view switching (no React Router) |
| **样式** | Tailwind CSS | 3.4.17 | Utility-first CSS with custom "cinema" theme |
| **富文本编辑器** | TipTap | 3.22.3 | ProseMirror-based editor with custom extensions |
| **代码编辑器** | Monaco Editor | 4.7 | Code editing in frontstage |
| **图可视化** | ReactFlow | 11.11 | Knowledge graph display |
| **动画** | Framer Motion | 12.38 | UI animations |
| **图标** | lucide-react | 0.487 | Icon library |
| **数据库 (桌面端)** | SQLite + rusqlite | 0.39 | Local data persistence, 73 migrations |
| **向量库** | LanceDB | 0.27 | Semantic search embeddings |
| **数据库 (服务端)** | PostgreSQL | 16 | Multi-user auth backend |
| **ORM (服务端)** | SQLx | 0.8 | Type-safe SQL for Postgres |
| **认证** | JWT + OAuth2 | -- | Authentication flow |
| **测试 (单元测试)** | Vitest | 4.1.4 | Frontend unit tests |
| **测试 (E2E)** | Playwright | 1.59.1 | End-to-end tests |
| **CI/CD** | GitHub Actions | -- | 4-job pipeline: rust-check, frontend-check, e2e-check, tauri-build |

---

## 4. 优点

1. **结构清晰的 monorepo**: Tauri 桌面端、服务端后端和 Web 前端之间分离清晰，每个层级有独立的构建配置。

2. **全面的领域建模**: SQLite 架构包含 73 个迁移文件，覆盖小说创作的广泛概念——故事契约、阅读力/追债、风格DNA、伏笔/回报账本、实体提及、流水线草稿/审核、知识图谱关系。这是一个经过深思熟虑的创作写作工具。

3. **事件驱动架构**: 通过 Tauri 事件（SyncEvent, FrontstageEvent, BackstageEvent）的发布/订阅模式，在幕前和幕后窗口之间提供清晰的分离而不紧耦合。

4. **Rust 中结构化的错误处理**: `AppError` 枚举带类型化变体、`From` trait 实现和基于代码的恢复 UI 是良好的模式。

5. **完善的日志系统**: 结构化 JSON 文件日志带每日轮转、前端日志 IPC 桥接和日志清理，生产级别。

6. **现代模式的良好运用**: Repository 模式做数据访问、Zustand 做状态管理、TanStack Query 做缓存、自定义 hooks 封装业务逻辑。

7. **流水线系统**: blueprint -> draft -> revision -> review -> finalize 流水线带 `post_process_runs/steps` 表，展示了对迭代内容生成的深思熟虑的设计。

8. **自定义 TipTap 扩展**: 角色名追踪、评论锚点、场景分隔符和修订追踪展示编辑器与领域模型的深度集成。

9. **MCP（模型上下文协议）支持**: 通过 MCP 集成外部工具，为高级用户提供可扩展性。

10. **双部署模型**: 可以作为本地优先的桌面应用运行，也可以部署在带 PostgreSQL 认证的服务端上，为不同用户场景提供灵活性。

---

## 5. 发现的问题

### 严重

1. **核心业务逻辑无测试覆盖**: 仅 5% 前端测试覆盖率和 0% Rust 后端覆盖率。核心 hooks（`useLlmStream`, `useSyncStore`, `usePipeline`）、stores 和所有 Rust 命令处理器都未测试。这是最大的风险——重构或修复 bug 时没有安全网。

2. **Rust 后端中 781 个 `unwrap()` 调用**: 静默吞掉错误，可能导致整个应用崩溃。`DB_POOL.lock().unwrap()` 模式意味着一个被污染的互斥锁（由先前的 panic 导致）将在每次后续请求时崩溃应用。

3. **未实现的 `get_current_user` 导致认证绕过**: Tauri auth 命令无条件返回 `Ok(None)`，任何检查登录状态的前端逻辑都会始终看到"未登录"。

### 中等

4. **XSS 漏洞**: AI 生成的内容通过 markdown 渲染时无 sanitization，可以包含 `javascript:` URL 和事件处理器。localStorage 存储 token（无 HttpOnly cookies）加剧了此问题。

5. **MCP 服务器启动的命令注入**: 用户可配置的命令路径在 `McpServer::start()` 中允许任意进程执行。`FileSystemTool` 允许对系统上任意文件进行不受限制的读/写。

6. **两个后端都有默认 JWT 密钥**: `storymoss-default-secret-change-me`（服务端）和 `storymoss-default-jwt-secret-change-in-production`（Tauri）是硬编码的后备方案，极易被利用。

7. **任何 API 端点都没有速率限制**: 启用暴力破解攻击、API 滥用和通过无限生成请求耗尽 LLM 成本。

8. **巨大的组件复杂度**: `FrontstageApp.tsx` 1800+ 行，`useSyncStore.ts` 200 行 switch 语句，`SceneEditor.tsx` 767 行——可维护性灾难。

9. **领域代码中 25 个 `any` 类型**: 在关键区域（TipTap 编辑器实例、API 响应和事件处理器）削弱了 TypeScript 安全性。

10. **集中化的 `handleError` 工具未使用**: `errorHandler.ts` 中设计良好的错误处理程序存在但从未被导入或使用——组件使用原始 try/catch。

### 轻微

11. **魔法数字散布在代码库中**: 硬编码的持续时间（3000, 2000, 8000, 10000, 30000ms）、阈值（0.8, 0.6, 0.4, 0.2）和限制（500 字符截断）应为常量。

12. **查询失效重复**: 相同的 9 个 `queryClient.invalidateQueries` 调用在 `App.tsx` 中出现两次。

13. **缺少对昂贵计算的 memoization**: `renderMarkdownToHtml` 在 LLM 流式传输期间每次渲染都运行，没有 `useMemo`。

14. **潜在的竞态条件**: LLM 流 `requestId` 碰撞风险，窗口恢复重试逻辑带硬编码 500ms 延迟。

15. **中英文注释混用**: 使非中文开发者难以维护。建议改用 git commit messages。

---

## 6. 安全漏洞

### HIGH 严重度（7 个发现）

| # | 发现 | 位置 | 影响 |
|---|------|------|------|
| 1A | 服务端默认 JWT 密钥 | `src-server/src/config.rs:31` | 用已知密钥伪造 JWT token |
| 1B | Tauri 默认 JWT 密钥 | `src-tauri/src/auth/session.rs:11` | 用已知密钥伪造 JWT token |
| 6A | `get_current_user` 始终返回 None | `src-tauri/src/auth/commands.rs:165` | 认证绕过——登录检查始终失败 |
| 6C | `dev_upgrade_subscription` 绕过付费 | `src-tauri/src/subscription/commands.rs:38-54` | 无需付款即可升级免费套餐 |
| 8A | MCP 服务器启动的命令注入 | `src-tauri/src/mcp/server.rs:241-256` | 用户系统上任意进程执行 |
| 8B | FileSystemTool 不受限制的文件操作 | `src-tauri/src/mcp/server.rs:27-61` | 读取/写入/删除用户系统上的任何文件 |
| 10A | 服务端端点无速率限制 | `src-server/src/main.rs` | 暴力破解、API 滥用、成本耗尽 |

### MEDIUM 严重度（10 个发现）

| # | 发现 | 位置 | 影响 |
|---|------|------|------|
| 3A | 无 sanitization 的 `dangerouslySetInnerHTML` XSS | `src-frontend/src/components/StreamOutput.tsx:252` | 在 AI 生成内容中执行恶意 JS |
| 4A | POST 端点无 CSRF token | `src-server/src/main.rs:47-51` | 如果 cookies 用于认证则 CSRF 攻击 |
| 5B | nginx 缺少 CSP/安全头 | `src-server-web/nginx.conf` | 无点击劫持、MIME 嗅探等保护 |
| 7A | 认证 token 存储在 localStorage（非 HttpOnly cookies） | `src-frontend/src/stores/useAuthStore.ts:42,48` | XSS 窃取 token |
| 9A | 文件上传无内容验证 | `src-tauri/src/book_deconstruction/service.rs:612-638` | 恶意文件执行（仅检查扩展名/大小） |
| 9B | 文件复制路径遍历 | `src-tauri/src/book_deconstruction/service.rs:82-91` | 写入意外位置的文件 |
| 10B | Tauri IPC 命令无速率限制 | Various | LLM 成本耗尽通过无限生成 |
| 12B | 无 JWT 撤销机制 | `src-server/src/auth/jwt.rs` | 被盗 token 在到期前有效（7天） |
| 12C | JWT 声明验证缺失（iss/aud） | `src-server/src/auth/jwt.rs:45` | 接受来自其他服务的 token |
| 13A | jsonwebtoken v9 带已知 CVE 模式 | Both Cargo.toml | 潜在 token 解析漏洞 |

### LOW 严重度（3 个发现）

| # | 发现 | 位置 | 影响 |
|---|------|------|------|
| 3B/3C | innerHTML 解析提取 HTML | `textAnalyzer.ts:22`, `SmartHintSystem.tsx:75` | 有限脚本执行风险 |
| 11A | OAuth 回调参数长度验证缺失 | `src-tauri/src/auth/commands.rs:84-157` | 超长字符串潜在内存问题 |
| 12A | JWT token 过期时间过长（7天） | Both backends | token 被攻陷后的暴露窗口延长 |

---

## 7. 各区域代码质量评分

| 类别 | 评分 | 评估 |
|------|------|------|
| **测试覆盖** | 2/10 | 前端 5%，Rust 后端 0%。核心业务逻辑完全未测试。 |
| **类型安全** | 6/10 | 领域代码中 25 个 `any`；否则合理的 TypeScript 用法，带泛型和判别联合。 |
| **错误处理** | 4/10 | Rust 中有良好的 `AppError` 枚举但 781 个 `unwrap()`；前端有未使用的集中化错误处理器。 |
| **架构** | 8/10 | 关注点分离清晰，良好的发布/订阅模式，模块结构组织良好。 |
| **可维护性** | 5/10 | 巨大组件（1800+ 行）、魔法数字、缺少昂贵计算的 memoization。 |
| **安全性** | 3/10 | 默认密钥、XSS 向量、命令注入、无速率限制、认证绕过。 |
| **性能** | 6/10 | 良好的代码分割策略；缺少 memoization 和虚拟化是中等关注点。 |
| **文档** | 6/10 | 全面的内联注释但中英文混用；无 API 文档。 |

---

## 8. 按影响排序的修复建议

### 第一阶段：安全修复（立即——2-4周）

1. **移除默认 JWT 密钥**: 替换硬编码后备方案为 `env::var("JWT_SECRET").expect("JWT_SECRET must be set")`，在两个后端中。
2. **实现 DOMPurify**: 添加 `dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(htmlContent) }}` 到 StreamOutput 和 sanitization 所有 innerHTML 赋值。
3. **迁移 token 到 HttpOnly cookies**: 替换 localStorage token 存储为 HttpOnly、Secure、SameSite=Strict cookies，缓解 XSS 影响。
4. **沙盒化 MCP 命令**: 对 MCP 服务器可执行文件路径添加白名单验证，限制 FileSystemTool 到沙盒目录。
5. **实现速率限制**: 在服务端添加 `actix-ratelimit` 中间件，在 Tauri IPC 处理器中添加命令级速率限制（尤其是 LLM 生成）。
6. **修复 `get_current_user`**: 实现正确的会话验证，从前端接受 token 并针对存储的 sessions 表验证。

### 第二阶段：核心测试基础（1-2个月）

7. **编写 Rust 后端测试**: 从最关键的命令处理器开始（认证、LLM 生成、场景 CRUD）。使用 `rusqlite` 内存数据库进行测试。
8. **为核心 hooks 添加单元测试**: `useLlmStream`、`useSyncStore`、`usePipelineProgress` 和 `useAiOperations` 是最高风险的未测试区域。
9. **测试 Zustand stores**: 为所有 5 个 stores 的变异逻辑、选择器正确性和持久化行为编写测试。
10. **添加认证流的集成测试**: 用模拟提供商端到端测试 OAuth2 + JWT 流。

### 第三阶段：Rust 后端加固（1-2个月）

11. **替换 `unwrap()` 调用**: 系统性地替换 781 个 `unwrap()` 调用为使用 `?` 和 `AppError` 枚举的正确错误传播。从最关键路径开始（DB 操作、LLM 调用）。
12. **添加互斥锁中毒处理**: 替换 `DB_POOL.lock().unwrap()` 为 `.map_err(|e| ...)` 以优雅地处理被污染的互斥锁，而非崩溃。
13. **实现 JWT 撤销**: 添加 token 黑名单表或使用短期访问 token + 刷新 token。

### 第四阶段：组件重构（2-3个月）

14. **拆分 FrontstageApp.tsx**: 提取事件监听器设置、生成逻辑和流水线命令到独立的 hooks/组件。目标：<500行每组件。
15. **分解 useSyncStore switch 语句**: 为每个实体类型创建单独的处理函数（如 `handleStorySync`、`handleCharacterSync`），而非一个 200 行的 switch。
16. **提取 SceneEditor 标签页**: 将 767 行组件拆分为独立的标签页组件，带各自的 hooks 和表单。

### 第五阶段：代码质量改进（持续）

17. **提取魔法数字为常量**: 创建 `constants.ts` 文件存放代码库中跨区域的持续时间、阈值和限制。
18. **添加 memoization**: 对 `renderMarkdownToHtml`、`countWords` 和 LLM 流式传输期间频繁变化的其他昂贵计算使用 `useMemo`。
19. **移除 `any` 类型**: 用适当的 TipTap/ProseMirror 类型、判别联合或泛型参数替换 25 个 `any`。
20. **使用集中化错误处理器**: 用从 `errorHandler.ts` 导入的 `handleError` 替换原始 try/catch 块。

---

## 9. 测试覆盖评估

### 当前状态

| 指标 | 值 |
|------|------|
| 前端源文件 | 174 |
| 有测试的前端文件 | 9 |
| **前端单元测试覆盖率** | **5.2%** |
| Rust 后端源文件 | 9 |
| 有测试的 Rust 文件 | 0 |
| **Rust 后端测试覆盖率** | **0%** |
| 总单元测试用例 | ~68 |
| 总 E2E 测试用例 | 15 |
| 总测试用例 | ~83 |
| 测试/源比例（前端） | 1:19 |

### 按类别覆盖

| 类别 | 源文件 | 已测试 | 覆盖率 |
|------|--------|--------|--------|
| hooks | 69 | 2 | 2.9% |
| frontstage/components | 18 | 3 | 16.7% |
| stores | 5 | 0 | 0% |
| services | 4 | 1 | 25% |
| utils | 5 | 1 | 20% |
| pages（全部） | 25 | 0 | 0% |
| extensions | 5 | 0 | 0% |
| Rust 后端 | 9 | 0 | 0% |

### 关键缺口

- **所有 5 个 Zustand stores**: 变异逻辑、选择器或持久化零测试。
- **核心 hooks**: `useLlmStream`（LLM 流式传输）、`useSyncStore`（实时同步）、`usePipelineProgress`（流水线编排）、`useAiOperations`（AI 生成）——全部未测试。
- **所有 TipTap 扩展**: 角色名追踪、评论锚点、场景分隔符、修订追踪——零测试。
- **设置页面**: 9 个子页面包括 UnifiedModelManager 和 ModelModal——零测试。
- **Rust 后端**: 任何地方零单元测试。`AppError` 枚举、SQLite 操作、LLM 集成和所有 100+ IPC 命令处理器都未测试。
- **E2E 测试**: 主要是基于截图的，断言极少；无认证流测试；使用 `waitForTimeout` 而非事件驱动等待（不稳定）。

### 测试质量备注

- `*.bug.spec.ts` 模式对于回归测试是优秀的，但仅存在 2 个（1 个被跳过）。
- `FrontstageApp.test.tsx` 中过度 mock（10+ 个模拟模块）创建了一个与生产几乎没有相似之处的测试环境。
- Tauri 模拟监听器无全局测试清理；`useSyncStore.bug.spec.ts` 中的共享可变状态脆弱。

---

## 10. 建议的重构优先级

### 优先级 1：安全（第 1-4 周）

```
src-server/src/config.rs          -- 移除默认 JWT 密钥后备
src-tauri/src/auth/session.rs     -- 移除默认 JWT 密钥后备
src-frontend/src/components/StreamOutput.tsx  -- 添加 DOMPurify sanitization
src-frontend/src/stores/useAuthStore.ts       -- 将 token 迁移到 HttpOnly cookies
src-tauri/src/mcp/server.rs           -- 沙盒化 MCP 命令执行 + FileSystemTool
src-server/src/main.rs               -- 添加速率限制中间件
src-tauri/src/auth/commands.rs       -- 正确实现 get_current_user
```

### 优先级 2：错误处理（第 5-8 周）

```
src-tauri/src/lib.rs                -- 替换 DB_POOL.lock().unwrap()
src-tauri/src/error.rs              -- 扩展 From<rusqlite::Error> 到所有错误变体
src-tauri/src/commands/*.rs         -- 系统性地替换 unwrap() 调用（从认证、LLM 开始）
src-frontend/src/utils/errorHandler.ts  -- 在组件中开始使用 handleError
src-frontend/src/App.tsx            -- 用集中化错误处理器替换原始 try/catch
```

### 优先级 3：组件分解（第 9-16 周）

```
src-frontend/src/frontstage/FrontstageApp.tsx  -- 拆分为 hooks/组件（每 <500 行）
src-frontend/src/hooks/useSyncStore.ts         -- 提取每个实体处理函数
src-frontend/src/components/SceneEditor.tsx    -- 拆分为标签页组件
src-frontend/src/frontstage/components/RichTextEditor.tsx  -- 移除 any 类型，添加适当的 TipTap 类型
```

### 优先级 4：测试基础（第 10-20 周）

```
src-tauri/src/tests/                -- 创建 Rust 测试模块结构
src-tauri/src/auth/commands.rs      -- 用内存 SQLite 测试认证命令
src-tauri/src/llm/                  -- 用模拟响应测试 LLM 集成
src-frontend/src/hooks/useLlmStream.test.tsx  -- 测试流式传输逻辑
src-frontend/src/hooks/useSyncStore.test.tsx   -- 测试同步事件处理
src-frontend/src/stores/appStore.test.ts     -- 测试 store 变异和选择器
```

### 优先级 5：代码质量（持续）

```
src-frontend/src/constants.ts       -- 提取魔法数字为常量
src-frontend/src/utils/cn.ts        -- 已有良好模式，扩展到其他工具函数
src-frontend/src/App.tsx            -- 提取查询失效到共享函数
src-frontend/src/frontstage/FrontstageApp.tsx  -- 为 renderMarkdownToHtml 添加 useMemo
```
