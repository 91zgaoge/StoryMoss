<p align="center">
  <img src="docs/images/logo.png" alt="StoryForge 草苔" width="120" />
</p>

# StoryForge (草苔) — AI 辅助小说创作系统

> 🌿 越写越懂的 AI 小说创作桌面应用
>
> 专为小说作者打造的创作工作台：幕后管理故事/角色/场景/世界观，幕前沉浸式写作，AI 在需要时随行辅助。

[![Version](https://img.shields.io/badge/version-v0.11.2-gold)](./CHANGELOG.md)
[![License](https://img.shields.io/badge/license-ISC-blue.svg)](./LICENSE)

**最新动态**：v0.11.2 紧急修复幕前 AI 续写长时间无反馈与模型管理删除不生效问题。后端为候选阶段增加整体超时、取消超时错误的重试（避免 120s×重试×候选数叠加到 500s 以上），并在预准备、候选生成、评估各阶段 emit 更细粒度事件；前端修复底部状态栏「系统仍在处理中...」重复追加 bug，并确保「生成候选 1/2」等具体进度不会被 LLM 心跳覆盖。同时修复设置页删除模型后 toast 成功但列表未刷新的状态不一致问题。完整报告见 [`CHANGELOG.md`](./CHANGELOG.md)。`cargo test` 332/332 通过，`vitest` 116 passed。

---

## 📖 用户指南

> 以下基于当前版本实际界面截图整理，持续更新。完整图文版见 [`docs/USER_GUIDE.md`](./docs/USER_GUIDE.md)。

### 一、产品概览

**草苔 StoryForge** 将创作流程分为两大空间：

| 空间 | 作用 | 适合场景 |
|---|---|---|
| **幕后（Backstage）** | 管理故事、角色、场景、世界观、AI 配置 | 规划、整理素材、配置模型 |
| **幕前（Frontstage）** | 沉浸式写作界面，专注正文创作 | 码字、与 AI 对话续写 |

核心思路：幕后把创作要素结构化管好，幕前让你专注写字，AI 在需要时介入，不打断心流。

---

### 二、幕前写作界面

![幕前写作](docs/product-screenshots/00_frontstage.png)

极简、全屏的写作环境，唯一目的就是让你专注码字。

#### 顶部状态栏

| 元素 | 作用 |
|---|---|
| **草苔** | 返回幕后 |
| **字数** | 当前章节字数 / 总字数 |
| **18px** | 当前字号，点击可调 |
| **色调** | 暖赭 / 冷青 / 琥珀 / 靛紫 四种配色 |
| **设置** | 打开设置 / 幕后工作室 |
| **温** | 文思模式切换 |

#### 中间编辑区

- 点击"开始写作…"即可输入。
- 支持富文本格式。
- 自动保存。

#### 底部 AI 输入栏

- 输入任意指令，例如"帮我续写下一段""把这段改得更紧张""加入一个意外转折"。
- 按回车或点击纸飞机发送。

#### 文思模式

点击右上角 **温** 切换 AI 介入程度：

- **被动**：只在发指令时响应。
- **主动**：适时给出萤火提示（下一句建议、情节提醒）。

---

### 三、全局导航

左侧边栏是所有功能的入口，任何页面都可以一键切换。

![仪表盘](docs/product-screenshots/01_dashboard.png)

| 按钮 | 作用 |
|---|---|
| **开幕前写作** | 快速打开「幕前写作」窗口 |
| **仪表盘** | 回到首页，查看统计与快捷入口 |
| **故事** | 管理所有故事项目 |
| **角色** | 管理登场角色与关系 |
| **世界构建** | 设定世界观、势力、规则 |
| **场景** | 管理场景（情节单元） |
| **知识图谱** | 可视化角色/地点/事件关系 |
| **技能** | 配置 AI 辅助技能 |
| **MCP** | 连接外部模型/工具 |
| **拆书** | 分析参考书籍结构 |
| **任务** | 查看后台 AI 任务队列 |
| **伏笔看板** | 追踪伏笔埋设与回收 |
| **叙事分析** | 诊断故事节奏与结构 |
| **Story System** | 高级契约与版本管理 |
| **用量统计** | AI 调用与 Token 消耗 |
| **写作统计** | 字数、时长、写作习惯 |
| **设置** | 模型、账号、通用偏好 |

---

### 四、仪表盘 — 创作起点

![仪表盘](docs/product-screenshots/01_dashboard.png)

打开应用后首先进入这里。核心元素：

- **快捷创建**：
  - **AI 创建故事** —— 输入一句话创意，AI 生成故事框架（含大纲、角色、场景）。
  - **手动创建** —— 自己填写标题、简介、类型，从零开始。
- **统计卡片**：故事数 / 角色数 / 场景数，点击可跳转。
- **GENESIS 运行记录**：显示 AI 自动生成任务的运行历史。
- **开始创作引导**：没有故事时，下方会出现"开始你的创作之旅"，提供 AI/手动两种创建入口。

**典型路径**：打开应用 → 仪表盘 → AI 创建故事 → 输入创意 → 进入「故事」页继续完善。

---

### 五、故事 — 作品管理中心

![故事页](docs/product-screenshots/02_stories.png)

"故事"是创作的顶层容器。一本小说、一个短篇，都是一个故事。

首次使用时页面为空，需要先创建故事。有数据后：

- 故事卡片/列表展示标题、类型、进度、最近编辑时间。
- **打开** / **编辑** / **删除** / **导出** 等操作。

选择一个故事后，左侧底部会显示"当前编辑"，角色、场景、世界观等页面自动切换到该故事的数据。

---

### 六、角色 — 人物资料库

![角色页](docs/product-screenshots/03_characters.png)

管理系统化的人物设定：

- **基本信息**：姓名、性别、年龄、外貌。
- **性格与背景**：性格标签、核心驱动力、出身、目标。
- **关系网络**：与其他角色的关系可视化。
- **AI 生成角色**：输入一句话，AI 扩展成完整人设。

这让 AI 在续写时严格遵循人设，避免"角色崩坏"。

---

### 七、场景 — 情节单元

![场景页](docs/product-screenshots/04_scenes.png)

"场景"是故事的最小情节单位，类似"一场戏"。

- 场景卡片：标题、所属章节、出场角色、地点、状态。
- **新增 / 编辑 / AI 扩写 / 排序**。
- 把"写一章"拆成"写几场戏"，降低创作心理压力。

---

### 八、世界构建 — 设定资料库

![世界构建](docs/product-screenshots/05_world_building.png)

存放世界观、势力、地理、规则等背景设定。支持分类浏览、AI 生成世界观、关联角色/场景。

保证奇幻/科幻/架空作品的设定不自相矛盾，防止 AI "吃书"。

---

### 九、知识图谱 — 关系可视化

![知识图谱](docs/product-screenshots/06_knowledge-graph.png)

把角色、地点、事件、势力变成一张可交互网络图：

- 拖拽节点、缩放画布。
- 点击节点查看详情。
- 筛选显示某类节点。

直观发现"谁太久没出场""哪条线索忘了回收"。

---

### 十、技能工坊 — AI 辅助技能

![技能页](docs/product-screenshots/07_skills.png)

管理和配置可复用的 AI 技能模板：

- **导入技能**：导入别人分享的技能配置。
- **分类筛选**：全部 / 写作 / 分析 / 角色 / 情节 / 风格 / 世界观 / 导出 / 集成 / 自定义。
- **技能卡片**：名称、描述、适用场景、启用开关。

在幕前写作时，可随时调用已启用的技能（如"续写""润色""生成大纲"）。

---

### 十一、MCP — 外部工具连接

![MCP](docs/product-screenshots/08_mcp.png)

MCP（Model Context Protocol）让草苔连接外部模型或数据源，扩展 AI 能力。例如连接专门的"古文润色"模型或私有知识库。

---

### 十二、拆书 — 学习经典结构

![拆书](docs/product-screenshots/09_book-deconstruction.png)

上传参考小说，AI 自动分析：

- 整体结构（三幕式、英雄之旅等）
- 章节节奏与高潮分布
- 角色出场频率
- 核心主题

把"凭感觉写"变成"有参照地写"。

---

### 十三、任务 — 后台作业队列

![任务页](docs/product-screenshots/10_tasks.png)

当 AI 执行批量操作（批量润色、整书生成）时，会在这里显示进度。

- **状态筛选**：全部 / 执行中 / 等待中 / 已完成 / 失败。
- **新建任务**：手动发起后台 AI 任务。

你可以关闭界面去做别的事，回来在任务页查看结果。

---

### 十四、伏笔看板 — 线索回收

![伏笔看板](docs/product-screenshots/11_foreshadowing.png)

管理伏笔的全生命周期：

- **已埋下 / 已回收 / 待回收 / 废弃** 四态看板。
- 创建伏笔时填写描述、预期回收章节、重要性。
- 关联到具体场景。

防止"开头精彩、结尾烂尾"，确保每条线索都有交代。

---

### 十五、叙事分析 — 结构诊断

![叙事分析](docs/product-screenshots/12_narrative-analysis.png)

用 AI 诊断故事的叙事健康度：

- 节奏曲线（每章紧张度变化）
- 角色戏份分布
- 情节密度（对话/动作/描写比例）
- AI 诊断建议

像给小说做体检，发现结构问题再针对性修改。

---

### 十六、Story System — 高级契约系统

![Story System](docs/product-screenshots/13_story-system.png)

高级用户功能：

- **契约树**：定义 AI 必须遵守的规则（如"主角不能死""保持第三人称"）。
- **版本记录**：类似 Git 的提交历史，可回溯故事版本。
- **运行时规则**：控制 AI 生成的行为边界。

让 AI 在长篇幅创作中保持高度一致性。

---

### 十七、用量统计与写作统计

![用量统计](docs/product-screenshots/14_usage-stats.png)

**用量统计**：AI 调用次数、Token 消耗、按模型/功能拆分。适合关注 API 成本的用户。

![写作统计](docs/product-screenshots/15_writing-stats.png)

**写作统计**：每日字数、活跃时段、连续创作天数、平均写作速度。帮助你建立稳定输出节奏。

---

### 十八、设置 — 模型与偏好

![设置页](docs/product-screenshots/16_settings.png)

配置 AI 模型和应用偏好：

- **模型管理**：添加、删除、测试 LLM 连接（聊天/嵌入/多模态/图像）。
- **Agent 配置**：为不同 AI Agent 分配模型。
- **创作方法论**：选择雪花法、英雄之旅等创作框架。
- **工作流**：配置自动化流程。
- **通用设置**：主题、语言、自动保存、字号、行高。
- **数据统计**：查看本地功能使用统计。
- **账号与登录**：管理账号和订阅。

**首次使用建议**：进入 **模型管理** → **添加聊天模型** → 填写 API 地址和 Key → 测试连接 → 完成后即可在幕前调用 AI。

---

### 十九、快速上手

第一次使用草苔，建议按以下顺序：

1. 打开应用 → 看到仪表盘。
2. 点击 **AI 创建故事** → 输入创意一句话 → 等待 AI 生成框架。
3. 进入「故事」页 → 确认新建的故事。
4. 进入「角色」页 → 添加 2-3 个核心角色。
5. 进入「场景」页 → 创建第一章的关键场景。
6. 点击左侧 **开幕前写作** → 在幕前界面写第一章。
7. 卡壳时用底部 AI 输入栏求助。
8. 返回幕后「叙事分析」查看结构诊断。

---

### 二十、常见状态

- **顶部红色提示条"无法连接到本地服务"**：表示前端未连上后端。请等待几秒后点击"重试"，或重启应用。
- **左下角"登录"**：未登录状态，点击可登录账号。
- **右上角更新通知**：有新版本时弹出，可选择安装或忽略。

---

## 🚀 安装与运行

### 下载预构建版本

 releases 页面提供 Windows / macOS 安装包，下载后直接安装即可。

### 从源码运行

需要安装 [Node.js](https://nodejs.org/)（推荐 20 LTS）和 [Rust](https://rustup.rs/)。
仓库通过 `rust-toolchain.toml` 固定 Rust 版本为 **1.95.0**，`rustup` 会自动下载对应工具链。

```bash
# 1. 克隆仓库
git clone https://github.com/91zgaoge/StoryForge.git
cd StoryForge

# 2. 安装前端依赖
cd src-frontend && npm install

# 3. 安装 Tauri CLI 并运行桌面应用
cd ..
npm install -g @tauri-apps/cli
cargo tauri dev
```

> **注意**：`Cargo.lock` 已纳入版本控制。如需升级依赖，请在本地验证 `cargo clippy` / `cargo test` 通过后再提交。

### 仅运行前端（开发调试）

```bash
cd src-frontend
npm run dev
```

然后在浏览器打开 `http://localhost:5173/`。

---

## 🏗️ 技术栈

- **前端**：React 18 + TypeScript 5.8 + Vite 6 + Tailwind CSS 3
- **桌面框架**：Tauri 2.4（Rust 后端 + Web 前端）
- **编辑器**：TipTap / ProseMirror
- **状态管理**：Zustand + TanStack Query
- **知识图谱**：ReactFlow
- **向量存储**：LanceDB + SQLite
- **LLM 适配**：OpenAI / Anthropic / Ollama / 自定义本地 API

---

## 📚 更多文档

| 文档 | 说明 |
|---|---|
| [`docs/USER_GUIDE.md`](./docs/USER_GUIDE.md) | 完整用户指南（含全部截图与详细说明） |
| [`CHANGELOG.md`](./CHANGELOG.md) | 版本更新日志 |
| [`ARCHITECTURE.md`](./ARCHITECTURE.md) | 系统架构设计 |
| [`AGENTS.md`](./AGENTS.md) | 开发代理指南 |

---

## 📸 截图清单

所有界面截图均由 CDP 自动截取，保存在 [`docs/product-screenshots/`](./docs/product-screenshots/)：

| 文件名 | 页面 |
|---|---|
| `00_frontstage.png` | 幕前写作 |
| `01_dashboard.png` | 仪表盘 |
| `02_stories.png` | 故事 |
| `03_characters.png` | 角色 |
| `04_scenes.png` | 场景 |
| `05_world_building.png` | 世界构建 |
| `06_knowledge-graph.png` | 知识图谱 |
| `07_skills.png` | 技能工坊 |
| `08_mcp.png` | MCP |
| `09_book-deconstruction.png` | 拆书 |
| `10_tasks.png` | 任务 |
| `11_foreshadowing.png` | 伏笔看板 |
| `12_narrative-analysis.png` | 叙事分析 |
| `13_story-system.png` | Story System |
| `14_usage-stats.png` | 用量统计 |
| `15_writing-stats.png` | 写作统计 |
| `16_settings.png` | 设置 |

---

## 🤝 参与贡献

欢迎通过 Issue 和 Pull Request 参与项目。大型改动建议先阅读 [`AGENTS.md`](./AGENTS.md) 和 [`ARCHITECTURE.md`](./ARCHITECTURE.md)。

---

<p align="center">
  Made with 🌿 by StoryForge Team
</p>
