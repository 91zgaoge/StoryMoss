# 记忆无处不在 — 记忆系统与智能创作流程深度融合设计方案

> 状态：待审批
> 目标：让记忆像空气一样充盈在智能创作的每一个环节，形成完整的记忆读写闭环
> 方案：混合式记忆路由器（本地过滤 + LLM 精排）+ 完整记忆生命周期管理

---

## 一、问题诊断

当前记忆系统与创作流程形成"两张皮"：

| 断裂点 | 现状 | 影响 |
|--------|------|------|
| 章节号错位 | auto_write 传 `chapter_number: None`，记忆始终按第1章构建 | 续写第5章时注入的是第1章记忆 |
| Orchestrator 盲区 | Writer→Inspector→Writer 闭环不感知记忆 | 质检无法检测角色状态突变/伏笔遗忘 |
| Pipeline 真空 | Refine/Review/Finalize 均未引用 memory | 审校阶段无法基于已有设定审稿 |
| 压缩师闲置 | MemoryCompressor 是独立命令，从未被创作流程自动触发 | 新内容不会自动压缩为记忆摘要 |
| 健康系统孤立 | RetentionManager 仅用于前端展示 | 记忆健康不影响创作策略 |
| smart_execute 无记忆 | PlanContext 无任何记忆字段 | 计划生成阶段零整合 |

---

## 二、核心架构

引入 **MemoryContext** 作为横切关注点，贯穿每个创作任务的全生命周期。

```
┌─────────────────────────────────────────────────────────────────────┐
│                        记忆-创作融合总架构                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  【读】创作前 ──→ MemoryRouter::route() ──→ 提取最相关记忆          │
│         ↑                                                            │
│    第一层：本地过滤（章节号/关键词/实体名/角色状态）                   │
│    第二层：LLM 精排（语义相关度 + 注入理由）                         │
│                                                                     │
│  【校验】创作中 ──→ Inspector 第7维「记忆一致性」                     │
│         ↑                                                            │
│    MemoryConsistencyChecker 对比生成内容与记忆条目                   │
│    检测：角色状态突变/伏笔遗忘/世界观冲突/时间线矛盾                  │
│                                                                     │
│  【写】创作后 ──→ MemoryCompressor::compress()                      │
│         ↑                                                            │
│    自动将新内容压缩为三层记忆摘要                                    │
│    更新：working_memory(近章) / episodic_memory(状态变更)            │
│           / semantic_memory(长期事实)                                │
│                                                                     │
│  【维护】后台 ──→ MemoryHealthDaemon                                 │
│         ↑                                                            │
│    定时归档遗忘实体 + 记忆健康报告 + 压缩旧记忆                       │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 三、组件设计

### 3.1 MemoryRouter — 混合式记忆路由器

```rust
pub struct MemoryRouter {
    pool: DbPool,
    llm_service: LlmService,
}

impl MemoryRouter {
    /// 主入口：两层过滤
    pub async fn route(
        &self,
        task_type: &str,           // "write" / "plan" / "review" / "dialogue"
        instruction: &str,         // 当前任务描述
        story_id: &str,
        chapter_number: i32,
        all_entries: Vec<MemoryEntry>,
    ) -> Result<Vec<ScoredMemoryEntry>, Error> {
        // 第一层：本地规则快速过滤
        let filtered = self.local_filter(task_type, instruction, chapter_number, all_entries);
        
        // 第二层：LLM 精排（只处理过滤后的候选集，通常从100条降到20-30条）
        let ranked = self.llm_rank(task_type, instruction, filtered).await?;
        
        // 按 MemoryBudget 截取 top N
        let budget = MemoryBudget::for_task_type(task_type);
        Ok(ranked.into_iter().take(budget.total_max()).collect())
    }
}
```

**第一层：本地过滤规则**

| 规则 | 条件 | 动作 |
|------|------|------|
| 章节邻近性 | 章节号距离 > 15 | 排除（除非为核心设定） |
| 关键词匹配 | instruction 中的角色名/地点/事件匹配记忆条目 | 保留 |
| 角色状态保护 | 条目为角色状态（`character_state:*`） | 始终保留 |
| 核心设定保护 | 条目 category 为世界观/核心规则 | 始终保留 |
| 归档排除 | `is_archived = true` | 排除 |
| 时间窗口 | 源章节号距当前 > 20 | 降低优先级 |

**第二层：LLM 精排 Prompt**

```
【任务】{task_type}: {instruction}
【待筛选记忆】（{count}条）
1. [{layer}] {source}: {content}
2. ...

请评估每条记忆与当前任务的相关度（0-100），并说明理由。
格式：{"id": 1, "score": 85, "reason": "涉及主角当前状态"}
只返回最相关的15条。
```

**路由策略配置**：
- `Fast`：只用第一层本地过滤（零额外延迟）
- `Precise`：两层都用（+1-3秒延迟，最高精度）
- `Adaptive`：根据 token 预算自动选择（默认）

### 3.2 MemoryContext — 任务级记忆上下文

```rust
pub struct MemoryContext {
    /// 本次注入的记忆（由 MemoryRouter 生成）
    pub injected_memories: Vec<ScoredMemoryEntry>,
    /// 记忆一致性报告（由 Inspector 生成）
    pub consistency_report: Option<MemoryConsistencyReport>,
    /// 待写入记忆系统的更新队列
    pub update_queue: Vec<MemoryUpdate>,
    /// 路由策略
    pub strategy: RoutingStrategy,
}

pub struct ScoredMemoryEntry {
    pub entry: MemoryEntry,
    pub relevance_score: f32,     // 0-100
    pub reason: String,           // 注入理由（如"主角状态变化"）
}

pub struct MemoryUpdate {
    pub layer: MemoryLayer,       // Working / Episodic / Semantic
    pub content: String,
    pub source_chapter: i32,
    pub entity_refs: Vec<String>, // 关联实体ID
}
```

### 3.3 MemoryConsistencyChecker — 记忆一致性校验

Inspector 新增第7维检查：

```
7. 记忆一致性（v0.8.0 新增）- 生成内容是否与已有记忆冲突
   - 角色状态一致性：角色属性/位置/状态是否与记忆一致
   - 伏笔回收状态：是否遗忘已 setup 的伏笔
   - 世界观规则遵守：是否违反世界观设定
   - 时间线连续性：事件顺序是否与已有记忆矛盾
```

**校验方式**：
- 不额外调用 LLM，复用 Inspector 已有的内容分析能力
- 将 `injected_memories` 中的角色状态/世界观规则/伏笔状态作为约束条件注入 Inspector prompt
- Inspector 在分析内容时，自然发现与记忆的冲突

**输出格式**：
```json
{
  "score": 82,
  "dimension_scores": {
    "logic": 18,
    "character": 20,
    "writing": 17,
    "pacing": 15,
    "world": 10,
    "style": 15,
    "memory": 12   // 新增
  },
  "memory_conflicts": [
    "角色'张三'在记忆中被设定为'受伤'，但本段写他'全力奔跑'",
    "伏笔'神秘信封'在第3章 setup，本段未提及回收"
  ]
}
```

### 3.4 MemoryWriter — 自动记忆写入器

创作完成后自动触发：

```rust
pub struct MemoryWriter {
    pool: DbPool,
    compressor: MemoryCompressorAgent,
}

impl MemoryWriter {
    /// 定稿后自动更新记忆
    pub async fn write(&self, story_id: &str, chapter_number: i32, content: &str) -> Result<(), Error> {
        // 1. 压缩内容 → 章节摘要
        let summary = self.compressor.compress(content).await?;
        
        // 2. 提取状态变更（角色/关系/世界）
        let deltas = self.extract_state_deltas(content).await?;
        
        // 3. 写入三层记忆
        self.update_working_memory(story_id, chapter_number, &summary).await?;
        self.update_episodic_memory(story_id, chapter_number, &deltas).await?;
        self.update_semantic_memory(story_id, &deltas).await?;
        
        // 4. 更新访问计数（用于记忆健康计算）
        self.touch_entities(&deltas).await?;
        
        Ok(())
    }
}
```

**触发时机**：
- 场景定稿（Finalize）完成后
- 自动续写每轮循环完成后
- 用户手动触发「更新记忆」按钮

### 3.5 MemoryHealthDaemon — 记忆健康守护进程

后台定时任务：

```rust
pub struct MemoryHealthDaemon;

impl MemoryHealthDaemon {
    /// 每小时运行一次
    pub async fn run_hourly(&self, pool: &DbPool) {
        // 1. 计算所有实体的保留分数
        let manager = RetentionManager::new();
        let report = manager.generate_retention_report(&entities);
        
        // 2. 自动归档遗忘实体（分数 < 0.2）
        let forgotten = manager.get_forgotten_entities(&entities);
        for (entity, _) in forgotten {
            repo.archive_entity(&entity.id)?;
        }
        
        // 3. 压缩陈旧记忆（超过30天未访问）
        self.compress_stale_memories(pool).await?;
        
        // 4. 发射健康报告事件到前端
        emit("memory-health-report", report)?;
    }
}
```

---

## 四、数据流详细设计

### 4.1 读路径：创作前记忆注入

```
AgentTask 创建
    ↓
build_agent_context() 或 ContextOptimizer
    ↓
MemoryRouter::route(task_type, instruction, story_id, chapter_number, all_entries)
    ├─ 第一层：local_filter() → 100条 → 30条
    ├─ 第二层：llm_rank() → 30条 → 15条（含分数+理由）
    └─ 按 MemoryBudget 截取
    ↓
MemoryContext { injected_memories: [...], ... }
    ↓
注入 AgentContext.memory_context
    ↓
build_writer_prompt() 中
    ├─ format_memory_pack_for_prompt() → 改为 format_memory_context()
    └─ 按【工作记忆】→【情景记忆】→【语义记忆】分层注入
    ↓
LLM 生成
```

**关键修复：章节号传递**
- `auto_write` 中：将 `chapter_number` 从 `None` 改为当前场景的实际章节号
- `smart_execute` 中：PlanContext 新增 `chapter_number` 字段

### 4.2 校验路径：创作中记忆一致性检查

```
Orchestrator::execute_full()
    ↓
Writer 生成初稿
    ↓
Inspector 质检
    ├─ 现有6维检查
    └─ 新增第7维：MemoryConsistencyChecker
        ├─ 读取 injected_memories 中的角色状态/世界观/伏笔
        ├─ 对比 current_content 中的角色行为/设定/事件
        └─ 输出 memory_conflicts[]
    ↓
Orchestrator 双轨平衡
    ├─ style_score / narrative_score
    └─ memory_score（新增）
    ↓
若 memory_score < 0.7：反馈中加入记忆冲突详情
```

### 4.3 写路径：创作后自动更新记忆

```
Finalize 完成 / auto_write 每轮结束
    ↓
PostProcessor::post_process()
    ├─ 现有：更新知识图谱/角色状态/向量索引
    └─ 新增：调用 MemoryWriter::write()
        ├─ MemoryCompressor::compress(content) → 摘要
        ├─ 提取 state_deltas → episodic 记忆
        └─ 更新 memory_items → semantic 记忆
    ↓
异步写入 SQLite
    ↓
发射 "memory-updated" 事件到前端
```

### 4.4 维护路径：后台记忆健康

```
定时任务（每小时）
    ↓
MemoryHealthDaemon::run_hourly()
    ├─ RetentionManager::calculate_retention_score() 对所有实体
    ├─ 归档遗忘实体（score < 0.2）
    ├─ 压缩陈旧记忆（>30天未访问）
    └─ 发射 "memory-health-report"
    ↓
前端 KnowledgeGraph 页面接收报告
    ├─ 显示记忆健康评分
    ├─ 显示建议归档的实体列表
    └─ 一键归档按钮
```

---

## 五、与现有系统集成

| 现有组件 | 改动点 | 工作量 |
|----------|--------|--------|
| `AgentContext` | 新增 `memory_context: Option<MemoryContext>` | ~5行 |
| `agents/mod.rs` | 导出 MemoryContext / ScoredMemoryEntry / MemoryUpdate | ~3行 |
| `agents/context_optimizer.rs` | build_full_context 中调用 MemoryRouter::route | ~20行 |
| `agents/service.rs` | format_memory_pack_for_prompt → format_memory_context | ~30行 |
| `agents/orchestrator.rs` | generate() 前后注入/提取 MemoryContext | ~15行 |
| `agents/commands.rs` | auto_write 传正确 chapter_number | ~5行 |
| `prompts/engine.rs` | Inspector prompt 新增第7维 | ~15行 |
| `commands/orchestrator.rs` | smart_execute 中 PlanContext 传 chapter_number | ~10行 |
| `planner/mod.rs` | PlanContext 新增 chapter_number / memory_context | ~5行 |
| `planner/executor.rs` | execute_writer 中注入 memory_context | ~10行 |
| `creative_engine/context_builder.rs` | build() 中调用 MemoryRouter | ~15行 |
| `agents/memory_compressor.rs` | 改为自动触发（remove dead_code allow） | ~5行 |
| `memory/orchestrator.rs` | 新增 build_memory_context() 方法 | ~30行 |
| `memory/retention.rs` | 新增定时任务入口 | ~10行 |
| `handlers.rs` | 注册新命令 | ~5行 |
| `stores/settingsStore.ts` | 新增记忆策略配置 | ~20行 |
| `pages/settings/GeneralSettings.tsx` | 新增「记忆策略」选项卡 | ~50行 |
| `pages/KnowledgeGraph.tsx` | 接收 memory-health-report 事件 | ~15行 |

**总计：约 18 个文件，~300 行改动**

---

## 六、前端用户体验

### 6.1 设置面板新增「记忆策略」

```
┌──────────────────────────────────────────┐
│ 记忆策略                                   │
├──────────────────────────────────────────┤
│ 路由模式                                  │
│ ○ 快速（只用本地过滤）                    │
│ ● 精准（本地过滤 + LLM 精排）            │
│ ○ 智能自适应（根据 token 预算自动选择）    │
├──────────────────────────────────────────┤
│ 记忆预算                                  │
│ 工作记忆：10 条    情景记忆：15 条        │
│ 语义记忆：30 条                           │
├──────────────────────────────────────────┤
│ 自动更新                                  │
│ ☑ 定稿后自动更新记忆                      │
│ ☑ 续写每轮后自动更新                      │
│ ☑ 自动归档遗忘实体                        │
│   归档阈值：20%（可滑动调节 10%-50%）     │
└──────────────────────────────────────────┘
```

### 6.2 幕前写作感知

- **续写时**：无感知变化（记忆注入在后台完成）
- **质检报告中**：新增「记忆一致」评分条（绿色=一致，橙色=轻微冲突，红色=严重冲突）
- **定稿后**：toast 提示「记忆已更新，3 条新记忆已入库」
- **记忆冲突时**：Inspector 反馈中显示具体冲突（如「角色张三状态矛盾」）

### 6.3 幕后知识图谱增强

- **记忆健康页签**：显示实时健康评分 + 建议归档实体列表
- **实体卡片**：显示访问次数 + 最近访问时间 + 保留分数
- **一键归档**：批量归档遗忘实体

---

## 七、实施计划

### Phase 0：基础设施（3-4小时）

| 文件 | 改动 | 说明 |
|------|------|------|
| `agents/mod.rs` | 新增 MemoryContext 数据结构 | 核心类型定义 |
| `memory/orchestrator.rs` | 新增 `build_memory_context()` | 路由入口 |
| `agents/service.rs` | `format_memory_pack_for_prompt` → `format_memory_context` | prompt 注入兼容 |

### Phase 1：读路径（4-5小时）

| 文件 | 改动 | 说明 |
|------|------|------|
| `memory/router.rs` 新增 | MemoryRouter 两层过滤实现 | 核心组件 |
| `creative_engine/context_builder.rs` | 集成 MemoryRouter | 章节号正确传递 |
| `agents/context_optimizer.rs` | 集成 MemoryRouter | 记忆上下文构建 |
| `agents/commands.rs` | auto_write 传正确 chapter_number | 修复章节号错位 |
| `commands/orchestrator.rs` | smart_execute 传 chapter_number | PlanContext 修复 |
| `planner/mod.rs` | PlanContext 新增字段 | 数据结构扩展 |
| `planner/executor.rs` | execute_writer 注入 memory_context | Pipeline 整合 |

### Phase 2：校验路径（3-4小时）

| 文件 | 改动 | 说明 |
|------|------|------|
| `prompts/engine.rs` | Inspector 新增第7维 | 记忆一致性 prompt |
| `agents/orchestrator.rs` | 新增 memory_score 参与双轨平衡 | 闭环质检 |

### Phase 3：写路径（4-5小时）

| 文件 | 改动 | 说明 |
|------|------|------|
| `agents/memory_compressor.rs` | 改造为自动触发模式 | 去除 dead_code |
| `memory/writer.rs` 新增 | MemoryWriter 自动更新三层记忆 | 核心组件 |
| `agents/orchestrator.rs` | generate() 后调用 MemoryWriter | 写入触发 |
| `agents/commands.rs` | auto_write 每轮后更新记忆 | 续写闭环 |

### Phase 4：维护路径（2-3小时）

| 文件 | 改动 | 说明 |
|------|------|------|
| `memory/health_daemon.rs` 新增 | MemoryHealthDaemon 定时任务 | 后台维护 |
| `memory/retention.rs` | 新增定时任务入口 | 现有组件增强 |

### Phase 5：前端（3-4小时）

| 文件 | 改动 | 说明 |
|------|------|------|
| `stores/settingsStore.ts` | 新增记忆策略配置 | 状态管理 |
| `pages/settings/GeneralSettings.tsx` | 新增「记忆策略」选项卡 | UI |
| `pages/KnowledgeGraph.tsx` | 接收 memory-health-report | 健康展示 |

**总计：约 21-26 小时，19 个文件，~350 行改动**

---

## 八、验收标准

| 测试项 | 通过标准 |
|--------|---------|
| 章节号正确传递 | auto_write 续写第5章时，working_memory 包含第2-4章摘要 |
| 混合路由精度 | LLM 精排返回的 top 10 条记忆中，≥7 条与用户判断"明显相关" |
| 记忆一致性检查 | Inspector 能检测出"角色状态矛盾"和"伏笔遗忘"两种冲突 |
| 自动记忆写入 | 定稿后 5 秒内，memory_items 表中新增 ≥3 条相关条目 |
| 记忆健康归档 | 运行 health daemon 后，score < 0.2 的实体被自动归档 |
| 前端无感知 | 快速模式下续写延迟增加 < 200ms |
| 完整闭环 | 续写 → 记忆注入 → 质检（含记忆一致）→ 定稿 → 自动更新记忆 → 后台归档，全程无需手动操作 |

---

## 九、风险与备选

| 风险 | 概率 | 应对 |
|------|------|------|
| LLM 精排增加延迟 | 中 | 默认使用本地过滤，LLM 精排为可选；自适应模式根据 token 预算动态选择 |
| 记忆写入竞争（多任务同时定稿） | 低 | 使用 SQLite 事务 + 行级锁 |
| 记忆膨胀（memory_items 无限增长） | 中 | RetentionManager 自动归档 + 压缩旧记忆 |
| 与现有上下文长度冲突 | 低 | MemoryContext 注入的记忆总长度受 MemoryBudget 严格控制 |
|  Inspector prompt 过长 | 低 | 第7维检查复用已有分析，不额外增加 prompt 长度 |

---

**等待审批。批准后开始 Phase 0 实施。**
