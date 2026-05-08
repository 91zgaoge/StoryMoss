# StoryForge v3.0 更新摘要

> 发布日期: 2025-04-12  
> 版本: v3.0.0  
> 提交: 66a63ef

---

## 📋 概述

StoryForge v3.0 是一次**重大架构调整**，包含四个核心方向的全面重构：

1. 🎪 **场景化叙事架构** - 以场景取代章节，戏剧冲突驱动
2. 🧠 **增强记忆系统** - 基于 llm_wiki 方法论，真正的"越写越懂"
3. 🤖 **AI 智能生成** - 引导式小说创建，卡片式要素选择
4. 📦 **工作室配置系统** - 每部小说独立配置，支持导入/导出

---

## 🎪 场景化叙事架构

### 核心理念
从传统的"章节"概念转向"场景"概念：

- **章节 (Chapter)** - 以时间/长度驱动的线性单元
- **场景 (Scene)** - 以戏剧冲突为核心的叙事单位

### 关键特性

| 特性 | 说明 |
|------|------|
| **戏剧目标** | 每个场景都有明确的叙事使命 |
| **外部压迫** | 环境、反派、事件对角色的压迫 |
| **冲突类型** | 6 种标准冲突类型 |
| **角色冲突** | 角色间的利益、价值观、情感冲突 |

### 6 种冲突类型
```rust
pub enum ConflictType {
    ManVsMan,        // 人与人
    ManVsSelf,       // 人与自我
    ManVsSociety,    // 人与社会
    ManVsNature,     // 人与自然
    ManVsTechnology, // 人与科技
    ManVsFate,       // 人与命运
}
```

### 新增组件
- **StoryTimeline** - 可视化场景序列，支持拖拽排序
- **SceneEditor** - 三标签页场景编辑器（基础/戏剧/内容）
- **Scenes Page** - 场景管理页面

---

## 🧠 增强记忆系统

基于 [karpathy/llm_wiki](https://github.com/karpathy/llm_wiki) 方法论实现。

### 四层架构

```
┌─────────────────────────────────────────┐
│  Layer 4: Multi-Agent Sessions          │
│  世界观/人物/文风/情节/场景/记忆助手      │
├─────────────────────────────────────────┤
│  Layer 3: Knowledge Graph               │
│  带权实体关系图谱 (strength 0-1)          │
├─────────────────────────────────────────┤
│  Layer 2: Vector Store                  │
│  CJK Bigram 语义检索                     │
├─────────────────────────────────────────┤
│  Layer 1: Raw Sources                   │
│  场景正文、角色设定、世界设定             │
└─────────────────────────────────────────┘
```

### 核心组件

#### 1. CJK Tokenizer
- Bigram 二元组分词
- 中日韩 Unicode 范围检测
- 针对中文语义优化

#### 2. Ingest Pipeline (两步思维链)
```rust
// Step 1: 分析阶段
let analysis = analyze_content(content).await?;
// 提取：实体、关系、事件、情感、伏笔

// Step 2: 生成阶段  
let knowledge = generate_knowledge(&analysis).await?;
// 生成：实体档案、关系强度评分
```

#### 3. Knowledge Graph (带权知识图谱)
- 实体类型：人物/地点/物品/组织/概念/事件
- 关系类型：朋友/敌人/家人/恋人/因果/从属等
- 关系强度：0-1 浮点数，动态计算

#### 4. Query Pipeline (四阶段检索)
1. **CJK 分词搜索** - Token 级别匹配
2. **图谱扩展** - 基于关系强度扩展相关实体
3. **预算控制** - Token 预算分配 (4K-1M 可配置)
4. **上下文组装** - 带引用编号的结构化输出

#### 5. Multi-Agent Sessions
6 种独立助手，各持独立 Wiki：
- WorldBuilding - 世界观助手
- Character - 人物助手
- WritingStyle - 文风助手
- Plot - 情节助手
- Scene - 场景助手
- Memory - 记忆助手

---

## 🤖 AI 智能生成

### NovelCreationAgent

专门用于小说创建的 AI Agent：

```rust
impl NovelCreationAgent {
    // 生成世界观选项（3个）
    async fn generate_world_building_options(user_input: &str);
    
    // 生成角色谱选项
    async fn generate_character_profiles(world: &WorldBuilding);
    
    // 生成文字风格选项
    async fn generate_writing_styles(genre: &str, world: &WorldBuilding);
    
    // 生成首个场景
    async fn generate_first_scene(context: &StoryContext);
}
```

### 创建向导流程

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│  类型输入   │ -> │  世界观选择  │ -> │  角色谱选择  │ -> │   文风选择   │
│ 灰色提示词  │    │  卡片式 3选1 │    │  卡片式 3选1 │    │  卡片式 3选1 │
└─────────────┘    └─────────────┘    └─────────────┘    └──────┬──────┘
                                                                 │
                                                                 ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                           生成首个场景                                   │
│                          进入创作界面                                    │
└─────────────────────────────────────────────────────────────────────────┘
```

### 交互设计

- **灰色提示词**: "小说类型：玄幻...商战...或随便定"
- **单击选择**: 选中卡片
- **双击编辑**: 进入编辑模式修改内容
- **右键菜单**: 重新生成、复制、删除

---

## 📦 工作室配置系统

### 配置架构

每部小说拥有独立的工作室配置：

```
~/.config/storyforge/
├── config.json              # 全局配置
└── studios/
    └── {story_id}/
        ├── studio.json          # 工作室主配置
        ├── llm_config.json      # LLM配置
        ├── ui_config.json       # 界面主题
        └── agent_bots.json      # Agent配置
```

### 导入/导出

- **导出格式**: `.storyforge` (ZIP)
- **选择性导入**: 勾选需要导入的配置模块
- **冲突处理**: 同名小说检测，提示覆盖或重命名

### 默认主题

- **幕前**: 温暖纸张主题 (#f5f4ed)
- **幕后**: 暗色影院主题

---

## 📁 新增文件清单

### Rust 后端 (33 个文件)

#### V3 命令集
- `src-tauri/src/commands_v3.rs` - 26 个新 Tauri 命令

#### V3 数据层
- `src-tauri/src/db/models_v3.rs` - V3 数据模型
- `src-tauri/src/db/repositories_v3.rs` - V3 Repository 层

#### 记忆系统
- `src-tauri/src/memory/mod.rs`
- `src-tauri/src/memory/tokenizer.rs` - CJK 分词器
- `src-tauri/src/memory/ingest.rs` - Ingest 管线
- `src-tauri/src/memory/query.rs` - Query 管线
- `src-tauri/src/memory/multi_agent.rs` - 多助手会话

#### AI 生成
- `src-tauri/src/agents/novel_creation.rs` - 小说创建 Agent

#### 工作室配置
- `src-tauri/src/config/studio_manager.rs` - 工作室管理器

### 前端 (9 个文件)

#### 组件
- `src-frontend/src/components/StoryTimeline.tsx` - 故事线视图
- `src-frontend/src/components/SceneEditor.tsx` - 场景编辑器
- `src-frontend/src/components/NovelCreationWizard.tsx` - 创建向导

#### Hooks
- `src-frontend/src/hooks/useScenes.ts` - 场景管理
- `src-frontend/src/hooks/useWorldBuilding.ts` - 世界构建
- `src-frontend/src/hooks/useStudioConfig.ts` - 工作室配置

#### 页面
- `src-frontend/src/pages/Scenes.tsx` - 场景管理页面

#### 类型
- `src-frontend/src/types/v3.ts` - V3 TypeScript 类型

---

## 📊 数据库 Schema 变更

### 新增表 (8 个)

| 表名 | 说明 |
|------|------|
| `scenes` | 场景表（主叙事单位） |
| `world_buildings` | 世界观表 |
| `world_rules` | 世界规则表 |
| `settings` | 场景设置表 |
| `writing_styles` | 文字风格表 |
| `kg_entities` | 知识图谱实体表 |
| `kg_relations` | 知识图谱关系表 |
| `studio_configs` | 工作室配置表 |

### 保留表
- `chapters` - 用于向后兼容

---

## 📈 完成度统计

| 模块 | 完成度 | 说明 |
|------|--------|------|
| 场景化叙事系统 | 100% | Scene 模型、StoryTimeline、SceneEditor |
| 增强记忆系统 | 95% | Ingest/Query Pipeline、Knowledge Graph |
| AI 智能生成 | 100% | NovelCreationAgent、创建向导 |
| 工作室配置 | 100% | 导入/导出、主题系统 |
| **v3.0 总计** | **98.75%** | 核心功能全部完成 |

---

## 🎯 后续计划

### v3.1.0 (短期)
- [ ] 向量存储完整集成 (LanceDB)
- [ ] 知识图谱可视化
- [ ] 场景版本历史

### v3.2.0 (中期)
- [ ] 云端同步
- [ ] 协作写作增强
- [ ] 插件市场

### v4.0.0 (长期)
- [ ] WebAssembly 前端
- [ ] 自研小模型
- [ ] 移动端支持

---

## 📚 相关文档

- [V3 架构计划](plans/ARCHITECTURE_V3_PLAN.md) - 详细设计文档
- [功能清单](FEATURES.md) - 完整功能列表
- [CHANGELOG](../CHANGELOG.md) - 版本变更记录
- [PROJECT_STATUS](../PROJECT_STATUS.md) - 项目状态

---

**StoryForge (草苔)** - 让创作更智能 🌿
