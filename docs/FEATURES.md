# StoryForge (草苔) v0.23.13 功能清单（历史档案）

> 按幕前幕后双界面架构整理，当前项目版本：**v0.23.13**
>
> **注意**：本文档为历史归档，初始编写于 v0.7.4，部分早期功能描述可能未同步最新变更。
> 完整最新功能请参考 [README.md](../README.md) 和 [PROJECT_STATUS.md](../PROJECT_STATUS.md)。

---

## 🎭 幕前 (Frontstage) - 沉浸式写作界面

### 核心设计理念
- **极简主义**：接近最终阅读体验的排版
- **沉浸式写作**：禅模式全屏无干扰
- **AI 辅助**：文思泉涌般的智能提示
- **暖色调设计**：护眼纸张质感 (#f5f4ed)

### 已实现的页面与组件

#### 1. 主界面 (FrontstageApp)
**文件**: `src/frontstage/FrontstageApp.tsx`

| 功能 | 状态 | 描述 |
|------|------|------|
| 双栏布局 | ✅ | 可折叠侧边栏 + 主编辑区 |
| 故事切换 | ✅ | 下拉选择当前故事 |
| 🆕 场景导航 | ✅ | 大纲式场景列表（替代章节） |
| 字数统计 | ✅ | 中文字符 + 英文单词实时统计 |
| 自动保存 | ✅ | 2秒无操作后自动保存 |
| 快捷键支持 | ✅ | F11 禅模式, Ctrl+Space AI, Ctrl+S 保存 |
| 幕后切换 | ✅ | 一键切换到后台管理界面 |
| 精简侧边栏 Dock | ✅ | 3 按钮：修订模式、批注、幕后切换 |

#### 2. 富文本编辑器 (RichTextEditor)
**文件**: `src/frontstage/components/RichTextEditor.tsx`

| 功能 | 状态 | 描述 |
|------|------|------|
| TipTap 编辑器 | ✅ | ProseMirror 内核，稳定可靠 |
| Markdown 快捷键 | ✅ | Ctrl+B/I, Ctrl+Shift+1-6 标题 |
| 浮动工具栏 | ✅ | 格式、历史、列表、引用工具 |
| AI 续写 | ✅ | Ctrl+Space 触发流式生成 |
| 生成预览 | ✅ | AI 生成内容预览，Tab 接受/Esc 拒绝 |
| 排版设置 | ✅ | 字号(12-32px)、行高(1.2-3.0)调节 |
| 禅模式 | ✅ | F11 全屏沉浸式写作 |
| 写作风格 | ✅ | 5种风格，后台设置统一管理 |
| 右键上下文菜单 | ✅ | 剪切/复制/粘贴、修订模式、批注、评论、古典评点、全选 |
| ~~文本内联批注~~ | 🗑️ | v4.1.0 已移除，改为幽灵文本/古典评点 |
| ~~评论线程~~ | 🗑️ | v4.1.0 已移除，AI 反馈以评点/续写形式呈现 |
| 修订模式 | ✅ | trackInsert/trackDelete 可视化变更追踪 |
| 古典评点 | ✅ | AI 金圣叹风格段落点评 |
| 角色卡片弹窗 | ✅ | 点击角色名弹出详情卡片 |
| ~~底部 LLM 对话栏~~ | 🗑️ | v4.1.0 已移除，改为幽灵文本 + 萤火随行 |

#### 3. 场景大纲 (SceneOutline)
**文件**: `src/components/StoryTimeline.tsx`

| 功能 | 状态 | 描述 |
|------|------|------|
| 场景列表 | ✅ | 显示所有场景，戏剧目标预览 |
| 拖拽排序 | ✅ | @dnd-kit 实现拖放重新排序 |
| 冲突类型标签 | ✅ | 显示场景冲突类型 |
| 内联编辑 | ✅ | 点击编辑场景标题 |
| 删除场景 | ✅ | 确认后删除 |
| 添加场景 | ✅ | 一键添加新场景 |
| 选中高亮 | ✅ | 当前场景高亮显示 |

---

## 🎬 幕后 (Backstage) - 创作工作室

### 核心设计理念
- **专业管理**：完整的创作资源管理
- **场景化叙事**：以场景为单位的戏剧冲突驱动
- **AI 工作流**：Agent + Skills + 记忆系统
- **深色主题**：电影感暗色调界面

### 已实现的页面

#### 1. 仪表盘 (Dashboard)
**文件**: `src/pages/Dashboard.tsx`

| 功能 | 状态 | 描述 |
|------|------|------|
| 统计卡片 | ✅ | 故事数、角色数、🆕 场景数 |
| 🆕 快速创建 | ✅ | 新建小说向导入口 |
| 最近编辑 | ✅ | 最近更新的故事 |
| 快捷入口 | ✅ | 快速打开故事库 |
| 欢迎界面 | ✅ | 空状态引导 |

#### 2. 故事库 (Stories)
**文件**: `src/pages/Stories.tsx`

| 功能 | 状态 | 描述 |
|------|------|------|
| 网格布局 | ✅ | 故事卡片展示 |
| 🆕 创建故事 | ✅ | 引导式创建向导 |
| 编辑故事 | ✅ | 修改故事信息 |
| 删除故事 | ✅ | 确认删除 |
| 选择故事 | ✅ | 选中后进入场景管理 |
| 类型标签 | ✅ | 科幻/奇幻/悬疑等类型标识 |
| 🆕 工作室配置 | ✅ | 导出/导入工作室配置 |

#### 3. 🆕 场景管理 (Scenes)
**文件**: `src/pages/Scenes.tsx`

| 功能 | 状态 | 描述 |
|------|------|------|
| 故事线视图 | ✅ | StoryTimeline 组件 |
| 场景编辑器 | ✅ | SceneEditor 三标签页 |
| 创建场景 | ✅ | 自动生成序列号 |
| 编辑场景 | ✅ | 戏剧目标、冲突类型编辑 |
| 删除场景 | ✅ | 确认删除 |
| 拖拽排序 | ✅ | 调整场景顺序 |
| 🆕 AI 场景生成 | ✅ | 生成下一个场景建议 |

#### 4. 知识图谱 (KnowledgeGraph)
**文件**: `src/pages/KnowledgeGraph.tsx`

| 功能 | 状态 | 描述 |
|------|------|------|
| ReactFlow 可视化 | ✅ | 力导向图谱，节点按类型着色 |
| 关系边渲染 | ✅ | 按强度显示粗细和透明度 |
| 实体搜索 | ✅ | 实时按名称搜索节点 |
| 类型筛选 | ✅ | 6 种实体类型快速过滤 |
| 双击聚焦 | ✅ | 双击节点平滑动画居中 |
| 实体详情面板 | ✅ | 展示属性、关联关系 |
| 实体就地编辑 | ✅ | 修改名称、增删改属性 |
| 记忆健康面板 | ✅ | 保留报告、自动归档建议 |
| 已归档页签 | ✅ | 查看和恢复已归档实体 |

#### 5. 角色管理 (Characters)
**文件**: `src/pages/Characters.tsx`

| 功能 | 状态 | 描述 |
|------|------|------|
| 角色卡片 | ✅ | 头像、名称、性格预览 |
| 🆕 AI 生成角色 | ✅ | 基于世界观生成角色谱 |
| 创建角色 | ✅ | 名称/背景表单 |
| 删除角色 | ✅ | 确认删除 |
| 空状态 | ✅ | 无角色时引导 |
| 故事关联 | ✅ | 显示当前故事的角色 |

#### 6. 技能管理 (Skills)
**文件**: `src/pages/Skills.tsx`

| 功能 | 状态 | 描述 |
|------|------|------|
| 技能列表 | ✅ | 分类标签展示 |
| 分类筛选 | ✅ | Writing/Analysis/Character/Plot等 |
| 启用/禁用 | ✅ | 开关控制 |
| 5个内置技能 | ✅ | 文风增强/情节反转/角色声音/情感分析/节奏优化 |
| 技能导入 | ✅ | 文件选择器导入本地技能文件 |
| 技能执行 | ✅ | 自动收集必填参数并运行 |

#### 7. MCP 配置 (Mcp)
**文件**: `src/pages/Mcp.tsx`

| 功能 | 状态 | 描述 |
|------|------|------|
| 服务器列表 | ✅ | MCP Server 配置展示 |
| 连接测试 | ✅ | 外部服务器连接与断开 |
| 工具调用 | ✅ | 内置工具与外部工具统一展示调用 |

#### 8. 设置中心 (Settings)
**文件**: `src/pages/Settings.tsx`

| 功能 | 状态 | 描述 |
|------|------|------|
| 多类型LLM配置 | ✅ | Chat/Embedding/Multimodal/Image |
| 提供商支持 | ✅ | OpenAI/Anthropic/Azure/Ollama/DeepSeek/Qwen等 |
| 预设模型 | ✅ | 常见模型快速选择 |
| API Key 管理 | ✅ | 密码输入，安全提示 |
| 模型参数 | ✅ | Temperature/Max Tokens/Dimensions |
| 设置导出 | ✅ | JSON 文件下载 |
| 设置导入 | ✅ | JSON 文件上传 |
| 🆕 编辑器设置 | ✅ | 写作风格/字体/字号/行高 |
| Agent映射 | ✅ | 配置持久化 + 模型路由逻辑已完成 |

---

## 🆕 v3.0 新功能

### 🎪 场景化叙事系统

#### 核心概念
- **Scene (场景)** - 戏剧冲突驱动的叙事单位
- **戏剧目标 (Dramatic Goal)** - 每个场景要完成的叙事使命
- **外部压迫 (External Pressure)** - 环境/反派/事件对角色的压迫
- **冲突类型 (Conflict Type)** - 11 种标准戏剧冲突

#### 数据模型
```rust
pub struct Scene {
    pub dramatic_goal: String,         // 戏剧目标
    pub external_pressure: String,     // 外部压迫
    pub conflict_type: ConflictType,   // 冲突类型
    pub character_conflicts: Vec<CharacterConflict>, // 角色冲突
    pub setting: Setting,              // 场景设置
}

pub enum ConflictType {
    ManVsMan,           // 人与人
    ManVsSelf,          // 人与自我
    ManVsSociety,       // 人与社会
    ManVsNature,        // 人与自然
    ManVsTechnology,    // 人与科技
    ManVsFate,          // 人与命运
    ManVsTime,          // 人与时间
    ManVsMorality,      // 人与道德
    ManVsIdentity,      // 人与身份
    FactionVsFaction,   // 群体冲突
}
```

#### 前端组件
- **StoryTimeline** - 可视化场景序列，拖拽排序
- **SceneEditor** - 三标签页编辑器（基础/戏剧/内容）
- **useScenes Hook** - 场景管理逻辑

### 🧠 增强记忆系统

#### 系统架构
```
Layer 4: Multi-Agent Sessions (多助手会话)
Layer 3: Knowledge Graph (知识图谱)
Layer 2: Vector Store (向量存储)
Layer 1: Raw Sources (原始内容)
```

#### CJK Tokenizer
- Bigram 二元组分词
- 中日韩 Unicode 范围检测
- 针对中文语义优化

#### 采摘 Pipeline（Ingest，两步思维链）
1. **分析阶段** - LLM 提取实体、关系、事件、情感
2. **生成阶段** - 生成结构化知识，计算关系强度

#### Knowledge Graph (带权知识图谱)
- Entity (实体) - 人物/地点/物品/概念
- Relation (关系) - 带 strength 字段 (0-1)
- 关系强度动态计算

#### Query Pipeline (四阶段查询)
1. **CJK 分词搜索** - Token 级别匹配
2. **图谱扩展** - 基于关系强度扩展
3. **预算控制** - Token 预算分配 (4K-1M)
4. **上下文组装** - 带引用编号输出

#### 多助手会话
- **WorldBuilding** - 世界观助手
- **Character** - 人物助手
- **WritingStyle** - 文风助手
- **Plot** - 情节助手
- **Scene** - 场景助手
- **Memory** - 记忆助手

### 🤖 AI 智能生成

#### NovelCreationAgent
- `generate_world_building_options()` - 生成世界观选项
- `generate_character_profiles()` - 生成角色谱
- `generate_writing_styles()` - 生成文字风格
- `generate_next_scene()` - 生成下一个场景

#### 创建向导流程
```
类型输入 → 世界观选择 → 角色谱选择 → 文风选择 → 生成首个场景
   ↑          ↑            ↑           ↑
  灰色      卡片式       卡片式      卡片式
  提示词    3个选项      3个选项     3个选项
```

#### 卡片式 UI
- 单击选择
- 双击编辑
- 右键重新生成

### 📦 工作室配置系统

#### 配置结构
```
~/.config/storyforge/studios/{story_id}/
├── studio.json          # 工作室主配置
├── llm_config.json      # LLM配置
├── ui_config.json       # 界面配置
└── agent_bots.json      # Agent配置
```

#### 导入/导出
- **导出** - `.storyforge` ZIP 格式
- **导入** - 选择性导入配置模块
- **冲突处理** - 同名小说检测

---

## 🔧 后端 (Rust/Tauri)

### 1. V3 数据层 (src/db)

| 模块 | 状态 | 描述 |
|------|------|------|
| models_v3.rs | ✅ | Scene/WorldBuilding/KnowledgeGraph 模型 |
| repositories_v3.rs | ✅ | V3 Repository 实现 |
| scenes 表 | ✅ | 场景表（主叙事单位） |
| world_buildings 表 | ✅ | 世界观表 |
| kg_entities 表 | ✅ | 知识图谱实体表 |
| kg_relations 表 | ✅ | 知识图谱关系表 |
| studio_configs 表 | ✅ | 工作室配置表 |

### 2. 记忆系统 (src/memory)

| 模块 | 状态 | 描述 |
|------|------|------|
| tokenizer.rs | ✅ | CJK Bigram 分词器 |
| ingest.rs | ✅ | 两步思维链采摘（Ingest）管线 |
| query.rs | ✅ | 四阶段查询检索管线 |
| multi_agent.rs | ✅ | 多助手会话管理 |

### 3. AI 生成 (src/agents)

| Agent | 状态 | 功能 |
|-------|------|------|
| NovelCreationAgent | ✅ | 小说创建专用 Agent |

### 4. 工作室配置 (src/config)

| 模块 | 状态 | 描述 |
|------|------|------|
| studio_manager.rs | ✅ | ZIP 导入/导出、冲突处理 |

### 5. V3 命令集 (commands_v3.rs)

| 类别 | 命令数 | 说明 |
|------|--------|------|
| 场景命令 | 12 | 场景的 CRUD 和排序 |
| 记忆命令 | 8 | 采摘（Ingest）/Query/Multi-Agent |
| 创建命令 | 4 | AI 生成相关 |
| 配置命令 | 2 | 工作室导入/导出 |
| **总计** | **26** | - |

### 6. 其他后端功能

| 模块 | 完成度 | 说明 |
|------|--------|------|
| LLM 集成 | 100% | OpenAI/Anthropic/Ollama |
| Agent 系统 | 95% | 5 种 Agent 完整实现 |
| 技能系统 | 100% | 内置 5 技能 + 扩展支持 |
| 向量检索 | 90% | TF-IDF + 语义检索框架 |
| 导出功能 | 100% | PDF/EPUB/Markdown |

---

## 📊 完成度统计

### v3.0 功能完成度

| 模块 | 完成度 | 备注 |
|------|--------|------|
| 场景化叙事系统 | 100% | Scene 模型、StoryTimeline、SceneEditor |
| 增强记忆系统 | 95% | 采摘（Ingest）/Query Pipeline、Knowledge Graph |
| AI 智能生成 | 100% | NovelCreationAgent、创建向导 |
| 工作室配置 | 100% | 导入/导出、主题系统 |
| 幕前界面 | 95% | 场景导航、编辑器 |
| 幕后界面 | 95% | 场景管理、创建向导 |
| 后端架构 | 100% | V3 数据层、命令集 |

### 综合完成度 (v5.5.1)

| 模块 | 完成度 | 备注 |
|------|--------|------|
| 场景化叙事系统 | 100% | Scene 模型、StoryTimeline、SceneEditor |
| 增强记忆系统 | 95% | 采摘（Ingest）/Query Pipeline、Knowledge Graph、LanceDB 向量索引 |
| AI 智能生成 | 100% | NovelCreationAgent、Bootstrap 两阶段、创建向导 |
| 工作室配置 | 100% | 导入/导出、主题系统 |
| 幕前界面 | 100% | 精简侧边栏、幽灵文本、`/` 菜单 |
| 幕后界面 | 100% | 场景管理、创建向导、知识图谱 |
| 幕前幕后自动关联 | 100% | Chapter↔Scene 双向映射、state_sync、实时同步 |
| 后台自动化 | 95% | Workflow 持久化、能力进化反馈环、向量索引闭环 |
| 后端架构 | 100% | V3-V5 数据层、命令集、语义搜索 |
| **整体项目** | **99%** | 核心功能全部完成，P1/P2 差距已修复 |

---

## 📝 待完善功能 (v5.5.1 状态)

### 高优先级
1. ~~向量存储完整集成~~ ✅ LanceDB 向量索引已集成，采摘（Ingest）后自动写入
2. ~~知识图谱可视化~~ ✅ ReactFlow 可视化已实现
3. ~~场景版本历史~~ ✅ 版本快照和回滚已实现

### 中优先级
4. ~~技能执行 UI~~ ✅ 前端技能执行已集成
5. ~~MCP 功能完善~~ ✅ MCP 连接测试、工具调用已实现
6. **统计分析** - 写作数据可视化增强

### 低优先级
7. **云端同步** - 数据备份和跨设备同步
8. **插件市场** - Skills 分享平台
9. **移动端适配** - 响应式布局优化
10. **多语言支持** - 国际化 i18n

---

## 🎯 使用建议

### v3.0 当前可用功能
- ✅ 完整的场景化叙事系统
- ✅ AI 引导式小说创建
- ✅ 卡片式世界观/角色/文风选择
- ✅ 故事线视图和场景编辑器
- ✅ 工作室配置导入/导出
- ✅ 记忆系统（采摘（Ingest）/Query）
- ✅ 沉浸式写作体验（幕前）

---

## 🆕 v0.7.x 新功能补充（本文档未覆盖的后续更新）

以下功能在本文档原始编写后陆续实现，详见 [README.md](../README.md) 和 [PROJECT_STATUS.md](../PROJECT_STATUS.md)：

### v0.7.0 - AI 三审 Pipeline + 角色动态状态 + 用量统计
- **AI 三审 Pipeline** — Refine/Review/Finalize + 后处理（知识库/笔记/角色状态/风格分析）
- **角色动态状态面板** — 6 字段动态追踪（位置/实力/身心状态/物品/事件），LLM 自动解析更新
- **用量统计看板** — 全局/单故事 LLM 调用统计 + 最近 20 条调用明细
- **幕前 Pipeline 指令** — `/` 菜单支持修稿/审稿/定稿直达 Pipeline
- **场景进度看板** — execution_stage 彩色徽章 + 多色进度条

### v0.7.2 - 存储同构化 + MCP 动态注册 + 聚合编辑
- **拆书存储同构化** — `reference_*` → `narrative_*` 统一存储
- **MCP 工具动态注册** — `CapabilityRegistry` 实时同步外部工具
- **1:N 聚合编辑 Schema** — `scene_commits` 新增 `chapter_id` 外键
- **SceneDividerNode** — TipTap 原子块节点，可视化场景边界
- **LLM 调用取消** — `request_id` 级精确取消
- **AppError 结构化 IPC** — 统一 `{ code, message, data }` 错误格式

### v0.7.3 - 商业模式重构 + 1:N 架构完成
- **功能订阅制** — 移除配额计量，改为 `has_feature_access` 功能开关（Free/Pro/Enterprise）
- **1:N Chapter↔Scene 架构** — 废弃 `chapters.scene_id`，全链路改为 `scenes.chapter_id`
- **SceneCommitService** — 提交粒度彻底对齐 Scene，`chapter_commits` → `scene_commits`
- **SceneDividerNode 预留接口** — `SceneDividerRepository` + `scene_divider_nodes` 表就绪

### v0.7.3+ - 高密度状态世界构建法
- **第五种创作方法论** — `high_density_world_building.rs` 完整实现 4 阶段世界构建
- **阶段 1：最小世界种子** — 高密度"世界切片" + 状态向量 + 桥节点
- **阶段 2：状态网扩张** — 主角群扩展 + 状态触发表 + 信息不对称矩阵
- **阶段 3：多线交织与回流** — 桥节点多线映射 + 回流点规划 + 事件多功能重用
- **阶段 4：密度迭代与克制** — 克制检查 + 未写出世界 + 涌现性验证 + 重读价值优化
- **前端集成** — `MethodologySettings.tsx` 新增选项 + 4 阶段选择 UI
- **Agent 接入** — writer prompt 自动注入状态向量、桥节点、事件回流等核心概念

---

## 📚 相关文档

- [V3 架构计划](plans/ARCHITECTURE_V3_PLAN.md) - V3 详细设计
- [CHANGELOG](../CHANGELOG.md) - 版本变更记录
- [PROJECT_STATUS](../PROJECT_STATUS.md) - 项目状态

---

## 🆕 v0.22.x 新功能补充（2026-06-21）

### v0.22.2 — 题材画像推荐资产种子 + 策略硬约束

- **GenreProfile 推荐种子**：`seed_genre_recommendations()` 为末世/科幻/修仙/都市/悬疑/历史 6 题材写入推荐风格+方法论+技能映射
- **策略选择器硬约束**：`build_selected_strategy` 中体裁画像有推荐时跳过 LLM 直接使用
- **算力档案默认值修正**：capability_score 未测试时默认 0.0（避免虚假质量分基准）

### v0.22.1 — 5 条建设性意见实施

- **StrategySelector 题材推荐映射**：`get_genre_recommendations()` 覆盖 7 种题材→风格推荐
- **StyleDNA 句长偏差检测**：`execute_time_sliced` 生成后检测句长偏差，>30% 记录建议
- **Inspector 方法论动态 prompt**：按 methodology_id 选择 prompt（5 种方法论全覆盖）
- **GenreProfile 推荐字段**：4 新列 + Migration 96 + Repository SQL 更新

### v0.22.0 — 提示词与后台资产完整结合

- **Phase A**：WriteTimeBundle 新增 4 字段，`to_prompt()` 追加 4 个 section
- **Phase B**：`build_inspector_prompt` 追加题材画像/方法论/角色状态/冲突/四元组
- **Phase C**：`generate_for_request_with_request_id` 新增 intent 参数，agent_type 自动推导意图
- **Phase D**：`select_candidates` 加载 CapabilityProfile 参与候选排序
- **Phase E**：新增 `asset_params.rs` —— StyleDNA→temperature / methodology→max_tokens / genre→max_tokens

### v0.21.0 — 提示词全量可配置化

- **注册表扩展**：新增 6 个 PromptCategory，注册 ~50 个新提示词条目
- **假接入修复**：15 个 key 改为 resolve_prompt（含 DB 覆盖）
- **旁路接线**：40+ 个硬编码提示词全部接入 registry
- **前端 PromptsPanel 重写**：Monaco 编辑器 + 批量导出/导入

### v0.20.x — SING 意图图集成

- **v0.20.1**：修复 5 处致命断环（资产同步/网关意图/执行图持久化/LLM 合成/PPR 传播）
- **v0.20.0**：arXiv:2606.16591v2 论文的意图-工具异构图理论全面集成，动态 ReAct + 分层发现

### v0.23.x 新功能补充（2026-06-22）

- **v0.23.0 TriShot 三击生成管线**：`GenerationMode::TriShot`、prompt_synthesis（manifest/synthesizer/refiner）、最快模型路由、BGP-2 智能改写、后台静默质检/改写/洞察
- **v0.23.1 架构债务清偿**：14 个全局单例移除、模块循环依赖斩断、`domain` 领域层扩展、`architecture_guard.py` 0 violations
- **v0.23.2 事件总线与状态同步治理**：`SyncEvent::ChapterCommitted`、前端 `content/isSaved` 收敛到 `frontstageStore`
- **v0.23.3 测试基线修复**：MigrationRunner 交错执行、V092 失败清零、`narrative_*` 表 `status` 列补齐
- **v0.23.4 智能层闭环落地**：LLM JSON mode（OpenAI/Ollama 原生结构化输出）、Review/Refine `refinement_notes`、MemoryPack 预算参数强类型化、拆书存储统一
- **v0.23.5 CI 格式化修复**：Rust nightly fmt + 前端 Prettier 格式化差异清零
- **v0.23.6 macOS 启动崩溃修复**：`VectorStore` State 初始化顺序调整，全平台 CI 构建通过
