# StoryMoss 代码质量完善与优化计划

> 基于 Brooks-Lint 全面扫描报告（v1.0）制定  
> 遗留问题：40 项（关键 12 / 警告 22 / 建议 6）  
> 已自动修复：14 项 Safe-class 问题

---

## 一、目标与原则

### 1.1 总体目标

在 **6 个迭代周期** 内，将代码库健康评分从 **32/100 提升至 75/100**，达到以下状态：

- 迁移系统标准化，消除 3,111 行手搓迁移框架
- 核心模块（Repository、Pipeline、LLM）具备回归测试保护
- 启动序列可插拔，新增子系统无需修改单一函数
- 前端 API 层按域拆分，消除 1,340 行上帝文件
- 领域模型从"贫血数据袋"演进为"富领域对象"

### 1.2 铁律（不可违反）

```
1. 每轮重构必须有测试覆盖，或先写测试再改代码
2. 绝不同时修改行为与结构；结构重构必须是纯行为保留的
3. 公共 API 变更需标注 BREAKING CHANGE 并更新调用方
4. 每轮结束后 `cargo test` + `tsc --noEmit` 必须全绿
```

### 1.3 优先级公式

```
优先级 = Pain(1-3) × Spread(1-3) × 阻塞系数

阻塞系数：
  - 阻塞其他阶段 = 3
  - 可并行 = 1
  - 独立 = 1
```

---

## 二、问题优先级矩阵

| 排名 | 问题 | 风险 | Pain | Spread | 阻塞 | 优先级 | 阶段 |
|------|------|------|------|--------|------|--------|------|
| 1 | 手搓迁移框架 3,111 行 | R1/R3/R4 | 3 | 3 | 3 | **27** | P1 |
| 2 | 测试覆盖率 3% | T5 | 3 | 3 | 3 | **27** | P1 |
| 3 | `run()` 532 行启动瓶颈 | R1/R2 | 3 | 3 | 2 | **18** | P2 |
| 4 | 全局可变静态变量群 | R5 | 3 | 3 | 2 | **18** | P2 |
| 5 | `tauri.ts` 1,340 行上帝文件 | R1 | 3 | 2 | 1 | **6** | P4 |
| 6 | `Scene` 30+ 字段贫血模型 | R6 | 2 | 3 | 1 | **6** | P3 |
| 7 | 领域层直接依赖 Tauri 基础设施 | R5 | 2 | 2 | 1 | **4** | P3 |
| 8 | 硬编码 IP 地址 | R1 | 2 | 1 | 1 | **2** | P4 |
| 9 | `PipelineOrchestrator` 懒类 | R4 | 2 | 2 | 1 | **4** | P3 |
| 10 | 前端 CRUD 重复模式 | R3 | 2 | 2 | 1 | **4** | P4 |

> **P1 = Phase 1（基础设施）, P2 = Phase 2（启动与状态）, P3 = Phase 3（领域层）, P4 = Phase 4（前端）, P5 = Phase 5（收尾）**

---

## 三、分阶段实施计划

### Phase 1：基础设施与测试根基（第 1-2 周）

**目标：** 建立标准化迁移框架 + 为核心模块铺设测试安全网。这是所有后续重构的前提。

#### 1.1 采用 `refinery` 迁移框架

**范围：** `src-tauri/src/db/connection.rs:833-3944`

**任务分解：**

- [ ] **T1.1** 添加 `refinery` 依赖到 `Cargo.toml`
- [ ] **T1.2** 创建 `migrations/` 目录，将 86 个内联迁移块提取为 `V001__*.sql` 文件
- [ ] **T1.3** 实现 `refinery::Migration` 适配器，兼容现有 `schema_version` 追踪
- [ ] **T1.4** 删除 `run_migrations()` 内联实现，替换为 `refinery::Runner::run()`
- [ ] **T1.5** 验证：
  - 新数据库能正确初始化
  - 现有数据库（schema_version > 0）能平滑升级
  - `cargo test` 全绿

**风险与缓解：**

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 迁移顺序与现有不一致 | 数据丢失 | 保留原 `schema_version` 对照表，逐个迁移比对 |
| 条件 DDL（IF NOT EXISTS）语义差异 | 升级失败 | 每份 SQL 文件先在手写脚本中验证 |
| `refinery` 与 bundled SQLite 兼容性 | 编译失败 | 先用 `cargo check` 验证，再写迁移 |

**预期成果：**
- `db/connection.rs` 减少 ~2,800 行
- 新增 ~86 个 `.sql` 迁移文件
- 消除 80 处复制粘贴的守卫模式

**验收标准：**
```
□ cargo test 全绿（264 passed）
□ 新数据库从零初始化成功
□ 现有开发数据库迁移后 schema_version 正确
□ db/connection.rs 行数 < 1,200
```

---

#### 1.2 为核心模块添加特征测试

**范围：** `SceneRepository`, `PipelineReviewExecutor`, `LlmService`

**任务分解：**

- [ ] **T1.6** 为 `SceneRepository` 创建 `src-tauri/src/db/repositories_tests.rs`（已存在，补充缺失路径）
  - 覆盖：`create_in_tx` 的章节自动创建逻辑
  - 覆盖：`get_by_story` 的 33 字段映射正确性
  - 覆盖：JSON 字段序列化/反序列化
- [ ] **T1.7** 为 `PipelineReviewExecutor` 创建 `src-tauri/src/pipeline/executor_tests.rs`
  - 使用 mock LLM 服务（不调用真实 API）
  - 覆盖：refine/review/finalize 三个操作分支
  - 覆盖：payload 缺失字段的错误处理
- [ ] **T1.8** 为 `LlmService` 创建 `src-tauri/src/llm/service_tests.rs`
  - 使用 `mockall` 或手动 stub 测试配置切换逻辑
  - 覆盖：provider 切换、超时处理、错误转换

**Mock 策略（避免 T4 Mock 滥用）：**

```rust
// 好：mock 的是外部边界（HTTP 请求），不是内部实现细节
struct MockLlmProvider { /* 预置响应 */ }
impl LlmProvider for MockLlmProvider { /* ... */ }

// 坏：不要 mock LlmService 自身的私有方法
```

**预期成果：**
- 新增 3 个测试模块，~30-40 个测试用例
- 测试覆盖率从 3% 提升至 15%
- 为 P2/P3 重构提供回归安全网

**验收标准：**
```
□ cargo test 新增测试全部通过
□ 新增测试能在无网络环境下运行（纯本地 SQLite + mock）
□ 每个被测模块至少覆盖：正常路径、错误路径、边界条件
```

---

### Phase 2：启动序列与全局状态治理（第 3-4 周）

**前提：** Phase 1 完成，测试安全网就位。

**目标：** 消除 `run()` 发散式变化热点，将全局状态封装进 Tauri 状态系统。

#### 2.1 提取 Bootstrap 插件系统

**范围：** `src-tauri/src/lib.rs:135-667`

**任务分解：**

- [ ] **T2.1** 定义 `BootstrapPlugin` trait：
  ```rust
  pub trait BootstrapPlugin: Send + Sync {
      fn name(&self) -> &'static str;
      fn bootstrap(&self, app: &mut App) -> Result<(), Box<dyn Error>>;
  }
  ```
- [ ] **T2.2** 将现有初始化逻辑提取为独立插件：
  - `DatabasePlugin`（DB init + seeding）
  - `VectorStorePlugin`（LanceDB 初始化）
  - `TaskSystemPlugin`（任务系统启动）
  - `AutomationPlugin`（自动化引擎）
  - `WorkflowEnginePlugin`（工作流引擎）
  - `MemoryDaemonPlugin`（内存守护进程）
  - `CapabilityPlugin`（能力演进）
  - `WindowManagerPlugin`（窗口生命周期）
- [ ] **T2.3** 在 `lib.rs` 中用插件注册表替换内联逻辑：
  ```rust
  let plugins: Vec<Box<dyn BootstrapPlugin>> = vec![
      Box::new(DatabasePlugin::default()),
      Box::new(VectorStorePlugin::default()),
      // ...
  ];
  for plugin in plugins {
      plugin.bootstrap(&mut app)?;
  }
  ```
- [ ] **T2.4** 确保初始化顺序通过 `plugin.priority()` 控制，而非代码位置

**风险与缓解：**

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 初始化顺序改变导致竞态 | 启动失败 | 保留原有顺序作为默认 `priority()`；逐个插件验证 |
| 插件错误处理不一致 | 部分初始化 | 每个插件必须支持 `rollback()`；失败时清理已初始化资源 |

**验收标准：**
```
□ cargo test 全绿
□ 应用能正常启动，所有子系统初始化成功
□ 新增子系统只需实现 trait 并加入注册表，无需修改 lib.rs
□ lib.rs 行数 < 200
```

---

#### 2.2 封装全局静态变量到 AppState

**范围：** `src-tauri/src/lib.rs:71-78,677-700`

**任务分解：**

- [ ] **T2.5** 定义 `AppState` 结构体：
  ```rust
  pub struct AppState {
      pub db_pool: DbPool,
      pub app_config: RwLock<AppConfig>,
      pub chapter_commit_debounce: DebounceMap,
      pub mcp_connections: RwLock<McpConnectionMap>,
      pub vector_store: Arc<VectorStore>,
      // ...
  }
  ```
- [ ] **T2.6** 将全局 `static` 替换为 `AppHandle.state::<AppState>()` 访问
- [ ] **T2.7** 修改所有依赖全局的模块，改为通过参数接收 `&AppState` 或 `AppHandle`
- [ ] **T2.8** 删除 `src-tauri/src/lib.rs` 中的全局 `static` 声明

**依赖关系：**
- 必须先完成 T2.1（BootstrapPlugin），因为插件需要 `AppState` 来存储初始化结果

**验收标准：**
```
□ 编译后 `grep -n 'static.*=' src-tauri/src/lib.rs` 返回 0 个全局可变状态
□ cargo test 全绿，且测试可并行运行（无全局状态竞争）
□ 启动时间变化 < 10%
```

---

### Phase 3：领域层重构（第 5-7 周）

**前提：** Phase 1-2 完成。

**目标：** 消除贫血模型，建立清晰的领域-基础设施边界。

#### 3.1 分解 `Scene` 贫血模型

**范围：** `src-tauri/src/db/models.rs`

**任务分解：**

- [ ] **T3.1** 分析 `Scene` 的 30+ 字段，按内聚性分组：
  - `SceneContent`：`title`, `content`, `word_count`, `status`
  - `SceneNarrativeProfile`：`narrative_intensity`, `narrative_sentiment`, `act_number`, `position_in_act`
  - `ScenePipelineState`：`execution_stage`, `style_dna_id`, `style_override`
- [ ] **T3.2** 创建值对象结构体，保留向后兼容的 `Serialize`/`Deserialize`
- [ ] **T3.3** 在 `Scene` 上添加领域方法：
  ```rust
  impl Scene {
      pub fn validate(&self) -> Result<(), ValidationError> { /* ... */ }
      pub fn is_in_act(&self, act: i32) -> bool { /* ... */ }
      pub fn estimated_reading_time(&self) -> Duration { /* ... */ }
  }
  ```
- [ ] **T3.4** 修改 `SceneRepository` 使用新的值对象
- [ ] **T3.5** 为 `WorldBuilding`, `Entity`, `Relation` 重复相同模式

**风险：** Schema 变更需要数据迁移。缓解：先不改数据库表结构，仅在 Rust 层重新组织内存表示。

**验收标准：**
```
□ Scene 字段数量从 30+ 减少到 < 12（其余移至内嵌值对象）
□ Scene 至少有 3 个行为方法（validate, is_in_act, estimated_reading_time）
□ cargo test 全绿
□ 数据库 Schema 暂时不变（仅内存模型重构）
```

---

#### 3.2 消除领域层对 Tauri 基础设施的直接依赖

**范围：** `src-tauri/src/agents/orchestrator.rs`

**任务分解：**

- [ ] **T3.6** 定义抽象 trait：
  ```rust
  pub trait EventBus: Send + Sync {
      fn emit(&self, event: &str, payload: impl Serialize);
  }
  pub trait Database: Send + Sync {
      fn pool(&self) -> &DbPool;
  }
  ```
- [ ] **T3.7** 为 Tauri 实现适配器：
  ```rust
  pub struct TauriEventBus { app_handle: AppHandle }
  pub struct TauriDatabase { pool: DbPool }
  ```
- [ ] **T3.8** 修改 `AgentOrchestrator::new()` 接收 `Arc<dyn EventBus>` 和 `Arc<dyn Database>`
- [ ] **T3.9** 修改测试使用 `MockEventBus` 和 `MockDatabase`

**验收标准：**
```
□ agents/orchestrator.rs 中无 `use tauri::` 导入
□ AgentOrchestrator 可在无 Tauri 运行时的情况下单元测试
□ cargo test 全绿
```

---

#### 3.3 消除 `PipelineOrchestrator` 懒类

**范围：** `src-tauri/src/pipeline/mod.rs`

**任务分解：**

- [ ] **T3.10** 分析所有调用 `PipelineOrchestrator` 的命令处理程序
- [ ] **T3.11** 将直接委托的方法内联到命令处理程序，或提升仓库方法为 public
- [ ] **T3.12** 若存在真正的领域逻辑（如事务协调），提取为 `PipelineService`
- [ ] **T3.13** 删除 `PipelineOrchestrator`

**验收标准：**
```
□ pipeline/mod.rs 中无 PipelineOrchestrator 定义
□ 所有原调用方编译通过
□ cargo test 全绿
```

---

### Phase 4：前端架构清理（第 6-8 周，可与 P3 部分并行）

**目标：** 分解上帝文件，建立清晰的类型-服务-组件分层。

#### 4.1 按域拆分 `services/tauri.ts`

**范围：** `src-frontend/src/services/tauri.ts:1-1340`

**任务分解：**

- [ ] **T4.1** 按业务域创建模块：
  ```
  src-frontend/src/services/
  ├── tauri.ts              # 仅保留 loggedInvoke 和 barrel re-export
  ├── api/
  │   ├── storyApi.ts       # 故事相关 IPC
  │   ├── characterApi.ts   # 角色相关 IPC
  │   ├── pipelineApi.ts    # 管道相关 IPC
  │   ├── memoryApi.ts      # 记忆相关 IPC
  │   ├── chapterApi.ts     # 章节相关 IPC
  │   └── settingsApi.ts    # 设置相关 IPC
  └── adapters/
      └── settingsAdapter.ts # 旧配置→新配置的映射
  ```
- [ ] **T4.2** 将内联领域接口（`StoryContract`, `ChapterCommit` 等）提取到 `src-frontend/src/types/`
- [ ] **T4.3** 更新所有导入路径（使用 IDE 全局重构）
- [ ] **T4.4** 将 `any` 类型逐步替换为严格类型

**验收标准：**
```
□ tauri.ts 行数 < 100（仅保留 loggedInvoke）
□ 无内联接口定义在 api/ 文件中
□ tsc --noEmit 全绿
□ 运行时功能测试通过（手动验证至少 3 个核心流程）
```

---

#### 4.2 提取 `App.tsx` 路由与事件监听

**范围：** `src-frontend/src/App.tsx:37-304`

**任务分解：**

- [ ] **T4.5** 创建 `src-frontend/src/hooks/useBackstageEvents.ts`
- [ ] **T4.6** 创建 `src-frontend/src/hooks/useWindowLifecycle.ts`
- [ ] **T4.7** 创建 `src-frontend/src/components/ViewRouter.tsx`
- [ ] **T4.8** 将 `App.tsx` 简化为：
  ```tsx
  export default function App() {
    useBackstageEvents();
    useWindowLifecycle();
    return <ViewRouter />;
  }
  ```
- [ ] **T4.9** 消除 `getState()` 直接突变，改用 store 选择器

**验收标准：**
```
□ App.tsx 行数 < 50
□ useBackstageEvents 和 useWindowLifecycle 各有独立测试
□ tsc --noEmit 全绿
□ 前端冒烟测试通过
```

---

#### 4.3 消除硬编码 IP 与重复模式

**范围：** `src-frontend/src/services/settings.ts`, `src-frontend/src/stores/appStore.ts`

**任务分解：**

- [ ] **T4.10** 创建 `.env.development`：
  ```
  VITE_FALLBACK_CHAT_API=http://10.62.239.13:17098/v1
  VITE_FALLBACK_MULTIMODAL_API=http://10.62.239.13:17099/v1
  VITE_FALLBACK_EMBEDDING_API=http://10.62.239.13:8089
  ```
- [ ] **T4.11** 修改 `settings.ts` 通过 `import.meta.env` 读取
- [ ] **T4.12** 创建 `createEntitySlice<T>` 工厂函数，消除 appStore.ts 中的 CRUD 重复
- [ ] **T4.13** 创建 `createMutationHook` 工厂函数，消除 useSettings.ts/useStories.ts 中的 mutation 重复

**验收标准：**
```
□ 源码中无硬编码 IP 地址（grep 验证）
□ appStore.ts 中 CRUD 逻辑行数减少 50%
□ useSettings.ts 中 mutation hook 定义行数减少 60%
□ tsc --noEmit 全绿
```

---

### Phase 5：后端代码质量收尾（第 8-9 周）

**目标：** 处理剩余的长函数和重复模式。

#### 5.1 分解 `db/connection.rs` 的 `create_tables()`

**范围：** `src-tauri/src/db/connection.rs:89-830`

**任务分解：**

- [ ] **T5.1** 按域拆分为 `create_story_tables()`, `create_scene_tables()`, `create_character_tables()`, `create_pipeline_tables()`, `create_memory_tables()`
- [ ] **T5.2** 每个辅助函数返回其 SQL 字符串
- [ ] **T5.3** `create_tables()` 按顺序拼接并执行

**验收标准：**
```
□ create_tables() 行数 < 50
□ 每个域辅助函数 < 150 行
□ cargo test 全绿
```

---

#### 5.2 提取 `db/repositories.rs` 的 `map_row_to_scene()`

**范围：** `src-tauri/src/db/repositories.rs`

**任务分解：**

- [ ] **T5.4** 提取 `fn map_row_to_scene(row: &Row) -> Result<Scene, rusqlite::Error>`
- [ ] **T5.5** 替换 `get_by_story()`, `get_by_chapter()`, `get_by_id()` 中的内联映射
- [ ] **T5.6** 验证 JSON 字段反序列化在所有路径上一致

**验收标准：**
```
□ Scene 映射逻辑只存在于一处
□ cargo test 全绿，特别是 repositories_tests.rs
```

---

#### 5.3 分解编排器长函数

**范围：** `src-tauri/src/agents/orchestrator.rs`

**任务分解：**

- [ ] **T5.7** 提取 `execute_full()` 中的 `run_feedback_loop()`
- [ ] **T5.8** 提取 `check_style_compliance()`
- [ ] **T5.9** 提取 `build_rewrite_prompt()`
- [ ] **T5.10** 提取 `generate_candidates()` 中的 `extract_reference_fingerprint()` 和 `score_candidate()`

**验收标准：**
```
□ execute_full() 行数 < 80
□ generate_candidates() 行数 < 50
□ 每个提取的函数有独立的逻辑内聚性
□ cargo test 全绿
```

---

## 四、时间线与里程碑

```
第 1-2 周 [Phase 1]  ████████░░░░░░░░░░░░  基础设施 + 测试根基
第 3-4 周 [Phase 2]  ░░████████░░░░░░░░░░  启动序列 + 全局状态
第 5-7 周 [Phase 3]  ░░░░████████████░░░░  领域层重构
第 6-8 周 [Phase 4]  ░░░░░░░░████████████  前端架构清理（与 P3 部分并行）
第 8-9 周 [Phase 5]  ░░░░░░░░░░░░░░████░░  后端收尾
第 9-10周 [验收]     ░░░░░░░░░░░░░░░░░░██  回归测试 + 文档更新
```

**关键里程碑：**

| 日期 | 里程碑 | 验收标准 |
|------|--------|----------|
| 第 2 周末 | P1 完成 | refinery 运行，新增 30+ 测试，覆盖率 15% |
| 第 4 周末 | P2 完成 | lib.rs < 200 行，全局 static 清零 |
| 第 7 周末 | P3 完成 | Scene 有行为方法，orchestrator 无 tauri 导入 |
| 第 8 周末 | P4 完成 | tauri.ts < 100 行，App.tsx < 50 行 |
| 第 10 周末 | 最终验收 | 健康评分 ≥ 75/100，cargo test + tsc 全绿 |

---

## 五、风险登记册

| 风险 | 可能性 | 影响 | 应对策略 |
|------|--------|------|----------|
| 迁移框架切换导致开发环境数据库损坏 | 中 | 高 | 切换前全员备份 `app.db`；提供一键重置脚本 |
| 插件系统引入初始化顺序 bug | 中 | 高 | 保留原有顺序作为默认；逐个插件灰度启用 |
| 前端拆分 API 文件导致导入错误 | 高 | 中 | 使用 TypeScript `paths` 别名 + IDE 全局重构 |
| 领域模型重构引入序列化不兼容 | 低 | 高 | 先不改 Schema，仅内存表示；加 serde 兼容性测试 |
| 测试编写耗时超预期 | 高 | 中 | P1 优先覆盖最关键路径；非关键路径可延至 P5 |
| 并行开发产生合并冲突 | 高 | 中 | 每轮限定修改范围；禁止跨 Phase 的大范围重构 |

---

## 六、度量指标

### 6.1 过程指标（每周跟踪）

```
□ cargo test 通过数 / 失败数
□ tsc --noEmit 错误数
□ clippy warning 数量趋势
□ 新增测试用例数
□ 代码行数变化（目标：核心文件减少 30%）
```

### 6.2 结果指标（每 Phase 结束）

```
□ 健康评分（Brooks-Lint /brooks-health）
□ 测试覆盖率（行覆盖 / 分支覆盖）
□ 平均函数长度（目标：Rust < 30 行，TS < 40 行）
□ 重复代码块数（SonarQube 或 cargo-dup）
□ 编译警告数（目标：从 302 降至 < 50）
```

---

## 七、决策日志

| 日期 | 决策 | 决策原因 | 替代方案 |
|------|------|----------|----------|
| - | 采用 `refinery` 而非 `diesel_migrations` | 更轻量，无需 ORM；与现有 raw SQL 风格兼容 | `diesel_migrations`：更重，需定义 schema DSL |
| - | Phase 3 不改数据库 Schema | 降低风险；先改内存模型验证设计 | 同时改 Schema：更快但风险更高 |
| - | Phase 4 与 Phase 3 部分并行 | 前端与后端领域层无直接依赖 | 串行执行：总时长 +2 周 |

---

## 八、批准签署

**计划编制：** Kimi Code CLI（Brooks-Lint）  
**日期：** 2026-06-08  
**版本：** v1.0

---

**批准人：** _________________  
**日期：** _________________  
**批准意见：**

□ **批准全部 5 个 Phase，按顺序实施**
□ **批准但调整优先级**（请说明）：_________________
□ **拒绝，需重新评估**（请说明）：_________________

---

> **下一步行动：** 待批准后，从 Phase 1 的 T1.1（添加 `refinery` 依赖）开始实施。
