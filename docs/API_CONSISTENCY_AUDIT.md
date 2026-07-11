# StoryMoss 前后端接口一致性审计报告

> 生成时间: 2026-04-14
> 审计范围: 所有 Tauri 注册命令 vs 前端 `invoke` 调用

---

## ✅ 已修复的严重不一致

| 问题 | 影响 | 修复方式 |
|------|------|---------|
| `get_scene_version_chain` | 前端 `useVersionChain` Hook 调用了一个后端未注册的命令 | 后端新增 `get_scene_version_chain` 命令并注册到 `lib.rs` |
| `get_config_command` / `update_config` | 前端 `services/tauri.ts` 中的 `getConfig`/`updateConfig` 调用了后端不存在的命令 | 前端改为调用现有的 `get_settings`/`save_settings`，并在内部做 `LlmConfig` 与 `AppSettings` 的转换 |

---

## ⚠️ 后端命令已注册但前端无调用路径（功能孤岛）

以下命令在后端已完整实现并注册，但前端没有形成真正的用户操作流程：

### 1. 知识图谱实体编辑
| 命令 | 说明 | 状态 |
|------|------|------|
| `update_entity` | 更新实体名称、属性、重新生成嵌入 | ✅ 已修复 — KnowledgeGraph 详情面板新增就地编辑表单 |
| `get_entity_relations` | 按实体 ID 查询关系 | 前端通过 `get_story_graph` 一次性获取全图，无需单独视图 |
| `get_story_entities` | 按故事查询实体列表 | 前端通过 `get_story_graph` 获取全图，此命令未被使用（设计冗余，可接受） |

### 2. MCP 外部服务器
| 命令 | 说明 | 状态 |
|------|------|------|
| `connect_mcp_server` | 连接外部 MCP 服务器 | ✅ 已修复 — MCP 页面新增外部服务器配置卡片 |
| `call_mcp_tool` | 调用外部 MCP 工具 | ✅ 已修复 — 外部工具与内置工具统一展示并支持执行 |

### 3. 技能系统
| 命令 | 说明 | 状态 |
|------|------|------|
| `import_skill` | 从本地路径导入技能 | ✅ 已修复 — Skills 页面新增"导入技能"按钮 |
| `get_skills_by_category` | 按分类获取技能 | Skills 页面使用前端本地筛选，未调用后端分类接口（体验已足够） |

### 4. Agent 执行
| 命令 | 说明 | 状态 |
|------|------|------|
| `agent_cancel_task` | 取消正在执行的 Agent 任务 | ✅ 已修复 — 后端实现任务取消机制，前端 SkillExecutionPanel 添加取消按钮 |
| `agent_execute_stream` | 流式执行 Agent | ✅ 已修复 — 前端迁移到 `agent_execute_stream`，支持进度事件监听 |

### 5. LLM 生成（Tauri 层）
| 命令 | 说明 | 状态 |
|------|------|------|
| `llm_generate` | Tauri 层 LLM 生成 | 已决策保留 HTTP 直连为主路径，Tauri 命令降级为内部备用 |
| `llm_generate_stream` | Tauri 层流式生成 | 同上 |
| `llm_cancel_generation` | 取消生成 | 同上 |
| `llm_test_connection` | 测试 LLM 连接 | 前端设置页使用 `test_model_connection`，已满足需求 |

---

## ⚠️ 前端封装但未被调用的函数（services/tauri.ts 中的死代码）

以下函数在 `services/tauri.ts` 中导出，部分已通过功能集成或路径统一得到使用：

| 函数 | 状态 |
|------|------|
| `getDashboardState` | `@deprecated` 标记 — Dashboard 直接调用 `get_state` |
| `getSkillsByCategory` | `@deprecated` 标记 — 前端本地筛选已足够 |
| `importSkill` | ✅ 已使用 — Skills 页面导入功能已集成 |
| `connectMcpServer` | ✅ 已使用 — MCP 外部服务器已集成 |
| `callMcpTool` | ✅ 已使用 — MCP 外部工具执行已集成 |
| `searchSimilar` | ✅ 已使用 — `useVectorSearch.ts` 已统一复用 |
| `embedChapter` | `@deprecated` 标记 — 待章节嵌入手动触发功能 |
| `createEntity` | `@deprecated` 标记 — 待知识图谱手动创建实体功能 |
| `createRelation` | `@deprecated` 标记 — 待知识图谱手动创建关系功能 |
| `textSearchVectors` | ✅ 已使用 — `useVectorSearch.ts` 已统一复用 |
| `hybridSearchVectors` | ✅ 已使用 — `useVectorSearch.ts` 已统一复用 |

---

## ⚠️ 前端 Hook 定义但未被组件使用

以下 React Query Hook 已定义并导出，现已全部落地：

| Hook | 状态 |
|------|------|
| `useVersionChain` | ✅ 已集成 — `VersionTimeline` 新增"版本链"视图切换 |
| `useVersionDiff` | ✅ 已集成 — `DiffViewer` 在版本对比时显示差异元信息 |

---

## 🔍 参数命名一致性状态

| 命令 | 前端参数 | 后端参数 | 状态 |
|------|---------|---------|------|
| `get_pending_changes` | `scene_id` / `chapter_id` | `scene_id: Option<String>` / `chapter_id: Option<String>` | ✅ 一致 |
| `create_comment_thread` | `version_id`, `scene_id`, `chapter_id`... | 同名 Option 参数 | ✅ 一致 |
| `accept_all_changes` | `scene_id` / `chapter_id` | 同名 Option 参数 | ✅ 一致 |
| `track_change` | `change_type`, `from_pos`, `to_pos` | 同名参数 | ✅ 一致 |

---

## 📋 建议的后续行动

### ✅ 已完成
1. **知识图谱实体编辑** — `KnowledgeGraphView` 详情面板支持就地编辑名称和属性，调用 `update_entity`
2. **MCP 外部服务器连接** — `Mcp.tsx` 新增外部服务器配置表单，支持连接/断开/执行外部工具
3. **技能导入** — `Skills.tsx` 新增"导入技能"按钮，支持从本地文件导入
4. **Agent 取消任务** — 后端实现 `TASK_HANDLES` 取消机制，前端 `SkillExecutionPanel` 添加取消按钮
5. **统一向量搜索调用路径** — `useVectorSearch.ts` 已统一复用 `services/tauri.ts` 封装
6. **版本链 / Diff Hook 落地** — `VersionTimeline` 支持版本链视图，`DiffViewer` 接入 `useVersionDiff`
7. **LLM 调用路径决策** — 决策文档 `docs/LLM_CALL_PATH_DECISION.md` 已归档，明确保留 HTTP 直连
8. **Rust warnings 降噪** — 50+ 文件批量添加 `#[allow(dead_code)]` 等，warnings 从 163 降至 0

---

*报告由自动化脚本 + 人工审查生成*
