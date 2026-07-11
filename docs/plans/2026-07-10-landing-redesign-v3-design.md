# StoryMoss 落地页重新设计 v3 文档

## 背景

用户对 v2 落地页的产品介绍仍不满意，主要反馈：
- 缺少项目 LOGO（v2 已加入，但需继续保留并强化）。
- 产品介绍不够深入，未充分引用项目文档中的真实能力。
- 未突出 StoryMoss 真正的特色优势。

本版本保留「极简东方书卷」视觉系统，根据 README、AGENTS.md、ARCHITECTURE.md 等文档重新组织页面结构并全面重写文案，重点突出：
1. **幕后 + 幕前双空间** — 专业长篇小说创作工作台。
2. **Genesis 创世** — 从一句话创意到可写世界。
3. **分时介入架构** — 写得快也审得深。
4. **长上下文防御与稳定性** — Context Rot 防御、四级错误恢复、本地运行。

## 设计目标

- 建立清晰的专业产品定位：不是通用 AI 写作工具，而是长篇小说创作的系统工作台。
- 通过项目文档中的真实能力（分时介入、Context Rot 防御、四级错误恢复、场景优先架构、提示词注册表等）支撑文案。
- 强化项目 LOGO 的品牌识别（Navbar / Hero / Footer）。
- 用真实产品截图降低理解成本。

## 视觉系统

继承 v2 设计：
- 底色：`#f8f6f1`（宣纸白）
- 主文字：`#1a1816`（浓墨）
- 次级文字：`#6b6560`（淡墨灰）
- 强调色：`#a83f2e`（朱砂）
- 标题字体：`LXGW WenKai` Regular
- 正文字体：系统无衬线栈
- 圆角：`2px`
- 纹理：全局宣纸噪点

LOGO 复用 `docs/images/logo.png`，在 Navbar、Hero、Footer 中展示。

## 页面结构

共 11 个区块：

1. **Navbar** — 左侧 LOGO + 品牌名，右侧锚点，CTA 按钮。
2. **Hero** — LOGO、主标题、副标题、信任小字、双 CTA。
3. **ValueProp** — 一句话核心价值。
4. **PainPoints** — 长篇小说创作的三大痛点。
5. **BackstageFrontstage** — 左右对比：幕后工作室 vs 幕前写作台。
6. **Genesis** — 创世四步流程。
7. **TimeSliced** — 分时介入三条时间线。
8. **WhyStoryMoss** — 三大技术优势（长上下文防御、稳定性、本地运行）。
9. **Features** — 6 项真实产品能力，每项配截图。
10. **QuickStart** — 三步上手。
11. **DownloadCTA + Footer** — 下载号召、平台标签、LOGO、版权信息。

## 文案设计

### Hero

- 主标题：**把一句话创意，变成一本有序的小说**
- 副标题：草苔 StoryMoss 是专为长篇小说作者打造的系统工作台。幕后管理角色、场景、世界观；幕前沉浸式写作；AI 随行辅助，但不抢戏。
- 信任小字：v0.26.58 · 本地运行 · 开源可审计
- CTA：免费下载桌面版 / 看 Genesis 如何工作

### ValueProp

> 草苔不是聊天式 AI，而是一套把「灵感 → 规划 → 写作 → 审校」串起来的长篇小说创作系统。

### PainPoints

- 标题：**写到中途，往往毁于细节**
- 角色写着写着崩了 —— 人设越写越散，前后言行不一致。
- 伏笔埋了忘了回收 —— 前期线索后期无踪，读者白期待。
- 世界观越写越矛盾 —— 设定越堆越多，互相冲突难以自洽。

### BackstageFrontstage

- 标题：**两个空间，各尽其职**
- 幕后工作室：管理故事、角色、场景、世界观；知识图谱可视化人物、地点、事件关系；伏笔看板追踪线索的埋下与回收；AI 模型、提示词、创作方法论配置。
- 幕前写作台：极简、全屏、自动保存，无干扰码字环境；底部输入栏随时调用 AI 续写、润色、改紧张感；文思模式切换 AI 介入程度；创作主权在你，AI 只在需要时随行。

### Genesis

- 标题：**从一句话创意，到可写的世界**
- 副标题：输入一句话，30–90 秒生成故事框架。Genesis 把灵感变成可执行的创作结构。
- 四步：
  1. 概念解析 —— 一句话创意 → 题材画像、核心冲突、世界锚点。
  2. 策略选择 —— 匹配雪花法、高密度世界构建等创作方法论。
  3. 开篇骨架 —— 主角目标、戏剧冲突、世界锚点。
  4. 生成正文 —— 自动进入幕前，第一章已就绪，可立即修改与续写。

### TimeSliced

- 标题：**写得快，也审得深**
- 写作时刻：秒出正文，只带最小必要约束，让灵感不被流程卡住。
- 审计时刻：后台 7 维 Inspector 异步审校，问题以红黄蓝标注回流编辑器，当场处理小债。
- 洞察时刻：定期产出叙事健康度报告，节奏、戏份、结构一目了然。

### WhyStoryMoss

- 标题：**为什么草苔能 hold 住长篇？**
- 长上下文不丢约束：Context Prioritizer 按关键程度排序系统提示，并在结尾双重锚定，缓解「Lost in the Middle」。
- 稳定压倒灵感：四级错误分类（Fatal / Retry / Degraded / UserAction）+ 自重复 8% 重试闸门 + 场景优先架构，让 AI 输出更可控。
- 本地运行，数据归你：Windows / macOS / Linux 桌面端，数据留在本地；开源项目，可审计。

### Features

6 项能力，每项配真实截图：
1. 故事与场景管理
2. 角色与世界观
3. 知识图谱与伏笔追踪
4. AI 续写与润色
5. 拆书与叙事分析
6. 提示词注册表与模型管理

### QuickStart

- 标题：**三步开始写**
- 下载安装桌面版
- 用 Genesis 创建故事
- 进入幕前，写下第一段

### DownloadCTA

- 标题：**开始你的第一本书**
- 描述：Windows / macOS / Linux 桌面版免费下载。本地运行，数据归你。
- CTA：立即下载

### Footer

- 左侧 LOGO + 草苔 / StoryMoss
- 中部版权：© 2026 StoryMoss · 草苔
- 右侧：GitHub / 用户指南

## 组件调整

- `Navbar`：保留 LOGO，微调锚点文案。
- `Hero`：保留 LOGO，重写副标题与信任小字。
- `ValueProp`：重写文案。
- `PainPoints`：保留结构，优化标题与描述。
- `BackstageFrontstage`：保留双栏结构，更新文案。
- `Genesis`：保留四步流程，更新文案。
- `TimeSliced`：保留三步流程，更新描述以贴近文档。
- `WhyStoryMoss`：新增组件，展示三大技术优势。
- `Features`：保留 6 项，更新文案与截图。
- `QuickStart`：更新步骤 2 文案为「用 Genesis 创建故事」，同步更新测试。
- `Footer`：加入 LOGO。
- `App.tsx`：按新结构组装，移除旧 `Solution`，新增 `WhyStoryMoss`。

## 截图需求

复用 `docs/product-screenshots/` 中的现有截图：
- `01_dashboard.png` — 幕后工作室仪表盘
- `00_frontstage.png` — 幕前写作台
- `02_stories.png` — 故事与场景
- `03_characters.png` — 角色与世界观
- `06_knowledge-graph.png` — 知识图谱
- `11_foreshadowing.png` — 伏笔看板
- `09_book-deconstruction.png` — 拆书
- `16_settings.png` — 提示词注册表与模型管理
- `docs/images/logo.png` — 品牌 LOGO

## 验收标准

- [ ] 视觉继承「极简东方书卷」方向。
- [ ] LOGO 出现在 Navbar、Hero、Footer。
- [ ] Hero / ValueProp / PainPoints / BackstageFrontstage / Genesis / TimeSliced / WhyStoryMoss 文案准确、来自项目文档。
- [ ] Features 6 项均配真实截图。
- [ ] `npx vitest run` 通过。
- [ ] `npx tsc --noEmit` 无错误。
- [ ] `npm run build` 成功。
- [ ] 部署到 ai.91z.net 后页面正常显示。
