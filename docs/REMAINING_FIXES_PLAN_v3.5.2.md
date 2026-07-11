# StoryMoss v3.5.2 剩余修复实施计划书

> 制定时间: 2026-04-22
> 基于: `docs/FULL_CODE_AUDIT_REPORT.md` 审计报告
> 前置条件: v3.5.1 已推送 GitHub，139 测试全部通过

---

## 一、项目概述

本次计划针对全面功能审计中发现的 **剩余 7 项问题** 进行修复。这些问题覆盖前端 UI 补全、后端架构完善、以及创作流程闭环。计划分为 **Phase D（核心功能闭环）** 和 **Phase E（体验优化）** 两个阶段实施。

---

## 二、问题清单与修复策略

### 问题 #14: 意图引擎接入聊天栏

| 属性 | 详情 |
|------|------|
| **优先级** | P1 |
| **复杂度** | 🔴 高 |
| **根因** | `RichTextEditor.tsx` 的 `handleSendMessage` 直接调用 `writerAgentExecute()`，完全绕过 `IntentParser` |
| **影响** | 聊天栏所有消息都被当作 Writer Agent 指令，用户无法调用 Inspector/OutlinePlanner/Character 等其他 Agent |

**修复方案:**
1. 在 `RichTextEditor.tsx` 中集成 `useIntent` Hook（已存在但未被使用）
2. `handleSendMessage` 流程改造:
   ```
   用户输入 → parse_intent IPC → IntentResult
     ├── intent = "write" → writerAgentExecute()
     ├── intent = "inspect" → 调用 Inspector Agent
     ├── intent = "outline" → 调用 OutlinePlanner Agent
     ├── intent = "character" → 调用 Character Agent（或回退到 Writer + 角色上下文）
     └── intent = "chat" / unknown → 保持当前 Writer 行为
   ```
3. 根据意图类型构建不同的 `AgentContext`（如 Inspector 需要注入当前全文，OutlinePlanner 需要注入世界观）
4. 聊天栏显示意图识别结果（小标签如"[质检]"/"[大纲]"/"[写作]"）

**验收标准:**
- 输入"帮我检查一下这段文字" → 调用 Inspector Agent
- 输入"设计一个故事大纲" → 调用 OutlinePlanner Agent
- 输入"续写接下来的内容" → 调用 Writer Agent
- 意图识别失败时回退到 Writer，不报错

**预估工时:** 3h

---

### 问题 #15: 技能系统补全 LLM 调用 + 缺失技能

| 属性 | 详情 |
|------|------|
| **优先级** | P1 |
| **复杂度** | 🔴 高 |
| **根因** | `execute_prompt` 中注释 "Actual LLM call would happen here"，直接返回 prompt JSON；缺失 2 个技能 |
| **影响** | 5 个技能中只有 `text_formatter` 真正可用；"文风增强""情节反转"点击执行后只返回 prompt 文本 |

**修复方案:**
1. **补全 `execute_prompt` LLM 调用**:
   - 在 `skills/executor.rs` 中注入 `LlmService`
   - `execute_prompt` 构建 prompt 后，调用 `llm_service.generate()` 获取实际 AI 输出
   - 返回 `SkillResult` 包含 AI 生成的内容

2. **补充缺失的 2 个技能**:
   - `builtin.character_voice` — 角色声音生成器
     - System Prompt: "你是一位角色声音设计师，根据角色设定生成该角色的标志性对话片段..."
   - `builtin.emotion_analysis` — 情感分析/节奏优化器
     - System Prompt: "你是一位情感分析专家，分析文本的情感弧线并提供节奏优化建议..."

3. **前端技能执行面板改造**:
   - 当前 `SkillExecutionPanel.tsx` 实际是 Agent 执行面板，与 Skills 系统无关
   - 修正为真正的技能执行面板，显示技能执行结果
   - 或统一走 `execute_skill` → `skill.execute()` → LLM 的调用链

**验收标准:**
- 5 个内置技能全部可执行并返回真实 AI 输出
- 文风增强器返回润色后的文本
- 情节反转生成器返回反转方案
- 情感分析器返回情感弧线分析

**预估工时:** 4h

---

### 问题 #16: MCP 持久连接 + 真实 web_search

| 属性 | 详情 |
|------|------|
| **优先级** | P1 |
| **复杂度** | 🟡 中 |
| **根因** | `connect_mcp_server` 创建客户端后立即 drop；`web_search` 返回模拟假数据 |
| **影响** | MCP 只是一个独立调试页，未集成到创作流程；无法提供真实检索增强 |

**修复方案:**
1. **MCP 持久连接**:
   - 在 `AppHandle` 状态中持久化 `McpClient`（`std::sync::Mutex<Option<McpClient>>`）
   - `connect_mcp_server` 保存连接到状态，返回工具列表
   - `call_mcp_tool` 从状态获取已有连接，复用而非重新创建
   - `disconnect_mcp_server` 命令显式断开连接

2. **真实 web_search**:
   - 方案 A: 集成 SerpAPI / DuckDuckGo API（需 API Key 配置）
   - 方案 B: 移除模拟 `web_search`，避免误导用户
   - 推荐方案 B（最小可行），后续版本再考虑真实搜索集成

3. **filesystem 工具集成到导出流程**:
   - 在 `export_story` 命令中，若 MCP filesystem 已连接，使用 MCP 工具写入文件
   - 或保持现有导出逻辑不变（降低风险）

**验收标准:**
- 连接 MCP 服务器后，`call_mcp_tool` 复用已有连接（不重新握手）
- `web_search` 返回模拟数据的提示明确标注"[模拟数据]"
- 或移除 `web_search` 工具

**预估工时:** 2.5h

---

### 问题 #17: auto_revise 添加取消/进度事件

| 属性 | 详情 |
|------|------|
| **优先级** | P1 |
| **复杂度** | 🟡 中 |
| **根因** | `auto_revise` 是阻塞同步调用，无 abort 机制；无进度事件 |
| **影响** | "full" 范围修改长篇小说可能耗时数分钟，无法中断；前端只有 spinner |

**修复方案:**
1. **改造成后台任务模式**（参考 `auto_write`）:
   - `auto_revise` 启动 `tokio::spawn` 后台任务
   - 使用 `TASK_HANDLES` 注册任务句柄
   - 任务中每完成一个阶段 emit 进度事件
   - 新增 `auto_revise_cancel` 命令

2. **进度事件设计**:
   - `auto-revise-progress-{task_id}`: `{ current_scope, total_scopes, stage: "reading"|"generating"|"saving" }`
   - `auto-revise-complete-{task_id}`: `{ revised_text, chars_changed }`
   - `auto-revise-error-{task_id}`: `{ error }`

3. **前端改造**:
   - `WenSiPanel` 修改面板显示进度条（如果是 "full" 范围）
   - 显示当前阶段（"读取文本 → 生成修改 → 保存结果"）
   - 提供"取消修改"按钮

**验收标准:**
- "full" 范围修改时，前端显示进度条和当前阶段
- 点击"取消"后任务优雅终止，编辑器内容不被修改
- 修改完成后 toast 提示"修改完成，共变更 X 字"

**预估工时:** 2.5h

---

### 问题 #18: StyleDNA 前端选择 UI

| 属性 | 详情 |
|------|------|
| **优先级** | P1 |
| **复杂度** | 🟡 中 |
| **根因** | 后台没有 StyleDNA 选择页面，幕前写作时无法切换风格 |
| **影响** | StyleDNA 后端已就绪（`style_dna_id` 注入 prompt），但用户无入口使用 |

**修复方案:**
1. **后台新增 StyleDNA 页面/组件**:
   - 路由: `/style-dna` 或集成到设置页
   - 显示 10 种内置风格卡片（金庸/张爱玲/海明威/村上春树/莫言/古典散文/现代极简/黑色侦探/武侠诗意/浪漫绮丽）
   - 每种风格显示六维雷达图（词汇/句法/修辞/视角/情感/对白）
   - 支持点击选择/取消选择

2. **保存到工作室配置**:
   - 选择的 `style_dna_id` 保存到 `studio_configs` 表
   - `AgentContext` 构建时自动读取并注入

3. **幕前写作界面风格指示器**:
   - 在 `FrontstageApp` 顶部状态栏显示当前选择的风格名称
   - 点击可快速切换到后台风格选择页

**验收标准:**
- 后台可查看 10 种内置风格的详细信息
- 点击选择后，幕前写作时 LLM 生成内容风格改变
- 选择状态持久化，重启后保持

**预估工时:** 3h

---

### 问题 #19: 工作流引擎前端"一键创作"按钮

| 属性 | 详情 |
|------|------|
| **优先级** | P1 |
| **复杂度** | 🟢 低 |
| **根因** | `run_creation_workflow` 后端命令已暴露，但前端无入口 |
| **影响** | 7 阶段全自动工作流无法从 UI 触发 |

**修复方案:**
1. **后台 Dashboard 新增"一键创作"卡片/按钮**:
   - 在 `Dashboard.tsx` 的"快速创建"区域旁添加
   - 点击弹出模态框，输入:
     - 故事创意（一句话描述）
     - 创作模式选择: "AI 全自动" / "AI 初稿 + 我精修" / "我先写 + AI 润色"
   - 提交后调用 `run_creation_workflow` IPC

2. **进度展示**:
   - 显示 7 阶段进度（构思 → 大纲 → 场景 → 写作 → 审阅 → 迭代 → 入库）
   - 每阶段完成后更新进度条
   - 完成后跳转至新创建的故事页面

3. **结果展示**:
   - 工作流完成后显示质量评分（结构/人物/风格/情节）
   - 提供"查看故事"和"继续编辑"按钮

**验收标准:**
- Dashboard 可见"一键创作"入口
- 输入创意后触发 7 阶段工作流
- 工作流完成后可在故事库中看到新作品
- 各阶段进度可实时查看

**预估工时:** 2h

---

### 问题 #20: 前端 confidence_score 类型补全

| 属性 | 详情 |
|------|------|
| **优先级** | P2 |
| **复杂度** | 🟢 低 |
| **根因** | 前端 `Scene` interface 缺少 `confidence_score` 字段 |
| **影响** | 前端无法显示/编辑场景的置信度分数 |

**修复方案:**
1. `src-frontend/src/types/v3.ts` 中 `Scene` interface 添加:
   ```typescript
   confidence_score?: number;
   ```
2. `SceneEditor.tsx` 三标签页的"基础"或"戏剧"标签中添加置信度滑块（0.0-1.0）
3. `Scenes.tsx` 场景卡片中显示置信度指示条（可选）

**验收标准:**
- Scene 类型包含 `confidence_score`
- 场景编辑器可设置置信度
- 保存后刷新仍显示正确值

**预估工时:** 0.5h

---

## 三、实施路线图

```
Week 1 (Day 1-3)
├── Day 1: 问题 #17 auto_revise 取消/进度 + 问题 #20 confidence_score
├── Day 2: 问题 #16 MCP 持久连接 + 问题 #19 一键创作按钮
└── Day 3: 问题 #18 StyleDNA 前端 UI

Week 2 (Day 4-6)
├── Day 4-5: 问题 #15 技能系统补全 LLM 调用 + 缺失技能
└── Day 6: 问题 #14 意图引擎接入聊天栏

Week 2 (Day 7)
└── 集成测试、文档更新、版本发布 v3.5.2
```

**总预估工时: 17.5h**

---

## 四、依赖关系

```
#14 意图引擎 ─┐
              ├──→ 均独立，无强依赖
#15 技能系统 ─┤
#16 MCP ──────┤
#17 auto_revise ─┤
#18 StyleDNA ──┤
#19 一键创作 ──┤
#20 confidence ─┘
```

所有 7 项可并行开发，推荐按"从简到难"顺序实施以快速获得正反馈。

---

## 五、验收清单

- [ ] 意图引擎: 聊天栏输入不同意图类型，正确路由到对应 Agent
- [ ] 技能系统: 5 个内置技能全部可执行并返回真实 AI 输出
- [ ] MCP: 持久连接复用，`web_search` 明确标注或移除
- [ ] auto_revise: full 范围显示进度条，支持取消
- [ ] StyleDNA: 后台可选择风格，幕前写作风格生效
- [ ] 一键创作: Dashboard 入口，7 阶段进度展示，结果可查看
- [ ] confidence_score: 场景编辑器可设置，持久化正确
- [ ] cargo test: 139/139 通过
- [ ] npm run build: 构建成功
- [ ] 版本号统一: Cargo.toml / package.json / tauri.conf.json → 3.5.2

---

## 六、风险与应对

| 风险 | 可能性 | 影响 | 应对 |
|------|--------|------|------|
| 意图引擎 LLM 解析准确率不高 | 中 | 用户意图被误分类 | 添加意图置信度阈值，低置信度回退到 Writer |
| 技能系统 LLM 调用增加成本 | 低 | Pro 用户 API 费用略增 | 技能执行添加配额检查 |
| MCP 持久连接内存泄漏 | 低 | 长时间运行后内存增长 | 添加连接超时自动断开机制 |
| StyleDNA UI 雷达图实现复杂 | 中 | 超出预估工时 | 先用文字列表替代雷达图，后续优化 |

---

*计划书由 Kimi Code CLI 根据全面功能审计结果制定*
*待用户审批后实施*
