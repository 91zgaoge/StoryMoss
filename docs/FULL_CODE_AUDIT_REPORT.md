# StoryMoss v3.5.0 全面功能审计报告

> 审计时间: 2026-04-22
> 审计范围: 后端 Rust + 前端 TypeScript，共 5 个模块
> 总计发现: **20 项问题**（6 项 P0 致命，14 项 P1 重要）

---

## 一、自动修改 (auto_revise) — 严重缺陷

| 优先级 | 问题 | 影响 |
|--------|------|------|
| **P0** | 修改结果**只被 console.log，永不应用到编辑器** | 用户点击"修改"，等待完成后编辑器内容完全不变 |
| **P0** | 修改结果**永不保存到数据库** | 即使手动复制结果，刷新页面后丢失 |
| P1 | 无进度/流式事件 | 大文本修改时前端只有 spinner，无进度反馈 |
| P1 | 无取消机制 | "full" 范围修改长篇小说可能耗时数分钟，无法中断 |
| P1 | 无 Ingest/知识图谱更新 | 修改后的内容不会触发知识图谱更新 |

**根因**: `WenSiPanel.tsx:185` 只有 `console.log`；后端 `auto_revise` 返回后即结束，无 `scene_repo.update`。

---

## 二、拆书功能 (book_deconstruction) — 严重缺陷

| 优先级 | 问题 | 影响 |
|--------|------|------|
| **P0** | **提取的书名/作者永不写入数据库** | 所有 txt/pdf 拆书结果列表显示"未命名" |
| **P0** | **convert_to_story 返回错误的 story_id** | 一键转故事后，角色/场景关联到不存在的 story_id |
| **P0** | 任务执行器**未调用 store_embeddings** | 通过任务系统分析的书籍不生成向量嵌入 |
| P1 | 任务完成时数据库进度停在 **95%** | 刷新页面后显示"未完成" |
| P1 | 心跳事件 progress=0 造成**UI 进度条闪烁** | 用户体验差 |
| P1 | 前端未处理 `cancelled` 状态标签 | 显示英文 "cancelled" |
| P1 | 前端任务事件**未过滤 task_id** | 同时多任务时进度乱跳 |
| P1 | `merge_short_chapters` 从未被调用 | 短章节未合并，chunk 质量不稳定 |

---

## 三、场景模型与版本控制 — 严重缺陷

| 优先级 | 问题 | 影响 |
|--------|------|------|
| **P0** | **`scene_versions` 表在生产环境中缺失** | 版本控制功能在生产环境完全不可用 |
| **P0** | **`conflict_type` 从错误列索引读取** (5 而非 6) | `external_pressure` 文本被误解析为冲突类型，几乎永远返回 `None` |
| P1 | 自动快照**忽略大部分字段变更** | 修改 `external_pressure`/`conflict_type`/场景设置 不会触发版本快照 |
| P1 | `create_scene` 命令**忽略戏剧字段** | 新建场景时 `dramatic_goal`/`external_pressure`/`conflict_type` 等全部为空 |
| P1 | `confidence_score` 缺失于前端 `Scene` 类型 | 前端无法显示/编辑置信度分数 |

---

## 四、创作工作流引擎 — 悬空代码

| 优先级 | 问题 | 影响 |
|--------|------|------|
| P1 | **CreationWorkflowEngine 从未被调用** | 7 阶段全自动工作流（构思→大纲→场景→写作→审阅→迭代→入库）完全无法使用 |
| P1 | **QualityChecker 未集成** | 四维质量评估（结构/人物/风格/情节）未在工作流中调用 |
| P1 | **无前端 UI** | 没有"一键创作"按钮或工作流触发界面 |

---

## 五、StyleDNA 系统 — 半实现

| 优先级 | 问题 | 影响 |
|--------|------|------|
| P1 | **10 种内置经典风格未自动写入数据库** | 新用户无法选择金庸/张爱玲/海明威等风格（`get_builtin()` 永远返回空） |
| P1 | **无前端风格选择 UI** | 后台没有 StyleDNA 选择页面，幕前写作时无法切换风格 |
| ✅ | 风格注入后端已就绪 | `execute_writer` 中 `style_dna_id` 存在时会注入 prompt 扩展 |

---

## 六、意图引擎 — 死代码

| 优先级 | 问题 | 影响 |
|--------|------|------|
| P1 | **前端聊天栏完全绕过意图解析** | 所有聊天输入都被当作 Writer Agent 指令，无法调用 Inspector/OutlinePlanner 等其他 Agent |
| P1 | **3 个 Agent 没有独立实现** | `character_agent`/`world_building_agent`/`memory_agent` 都回退到 Inspector/Writer |

---

## 七、技能系统 — 半实现

| 优先级 | 问题 | 影响 |
|--------|------|------|
| P1 | **仅 3/5 技能存在**（缺失角色声音、情感分析/节奏优化） | 文档承诺 5 个，实际只有 3 个 |
| P1 | **仅 text_formatter 真正调用 LLM** | 文风增强、情节反转只返回 prompt JSON，不产生实际 AI 输出 |
| P1 | `execute_prompt` 有 TODO 注释"Actual LLM call would happen here" | 技能执行器未完成 |

---

## 八、MCP 外部服务器 — 基本不可用

| 优先级 | 问题 | 影响 |
|--------|------|------|
| P1 | **无持久连接** | 每次调用创建新客户端、连接、断开，状态不保持 |
| P1 | **web_search 是模拟数据** | 返回假结果，无法提供真实检索增强 |
| P1 | **未集成到任何创作流程** | MCP 只是一个独立的调试页面 |

---

## 修复计划（按优先级排序）

### Phase 1: P0 致命缺陷修复（6 项）

| # | 修复项 | 预估工时 | 文件 |
|---|--------|---------|------|
| 1 | auto_revise 结果应用到编辑器 + 保存到数据库 | 2h | `WenSiPanel.tsx`, `commands.rs` |
| 2 | 拆书书名/作者保存到数据库 | 1h | `service.rs`, `executor.rs`, `repository.rs` |
| 3 | 拆书 convert_to_story 修复 story_id | 1h | `service.rs` |
| 4 | 拆书任务执行器调用 store_embeddings | 1h | `executor.rs` |
| 5 | 生产环境创建 scene_versions 表 | 1h | `connection.rs` + migration |
| 6 | 修复 conflict_type 列索引错误 | 0.5h | `repositories_v3.rs` |

### Phase 2: P1 功能补全（10 项）

| # | 修复项 | 预估工时 | 文件 |
|---|--------|---------|------|
| 7 | 拆书进度 95%→100%、心跳闪烁、cancelled 状态 | 1h | `analyzer.rs`, `executor.rs`, 前端 |
| 8 | 拆书前端任务事件过滤 task_id | 0.5h | `useBookDeconstruction.ts` |
| 9 | auto_revise 添加取消/进度事件 | 1.5h | `commands.rs`, `WenSiPanel.tsx` |
| 10 | 版本快照检测全部字段变更 | 0.5h | `Scenes.tsx` |
| 11 | create_scene 接受戏剧字段 | 0.5h | `commands_v3.rs`, 前端 |
| 12 | StyleDNA 内置风格自动种子化 | 1h | `connection.rs` migration |
| 13 | 意图引擎接入聊天栏 | 1.5h | `RichTextEditor.tsx`, `useIntent.ts` |
| 14 | 技能系统补全 LLM 调用 + 缺失技能 | 2h | `executor.rs`, `builtin.rs` |
| 15 | CreationWorkflowEngine 暴露为 Tauri 命令 | 1h | `commands_v3.rs` |
| 16 | 多项前端小修复 | 1h | `v3.ts`, `BookListGrid.tsx` 等 |

### Phase 3: 体验优化（4 项）

| # | 修复项 | 预估工时 |
|---|--------|---------|
| 17 | MCP 持久连接 + 真实 web_search | 2h |
| 18 | StyleDNA 前端选择 UI | 2h |
| 19 | 工作流引擎前端"一键创作"按钮 | 1.5h |
| 20 | 全面测试覆盖新增修复 | 2h |

**总预估工时: ~22 小时**
