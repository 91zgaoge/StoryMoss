# StoryMoss 落地页重新设计 v2 文档

## 背景

用户对 v1 落地页（极简东方书卷风格）的产品介绍不满意，主要反馈：
- 缺少项目 LOGO
- 产品介绍不够深入，未充分引用项目文档
- 未突出 StoryMoss 真正的特色优势

本版本保留 v1 的视觉系统（极简东方书卷），全面重写内容、结构与文案，重点突出：
1. **幕后 + 幕前双空间** — 专业长篇小说创作工作台
2. **Genesis 创世** — 从一句话创意到故事框架

## 设计目标

- 建立清晰的专业产品定位：不是通用 AI 写作工具，而是长篇小说创作的系统工作台
- 通过项目文档中的真实能力（分时介入、Context Rot 防御、四级错误恢复、场景优先架构等）支撑文案
- 加入项目 LOGO，强化品牌识别
- 用真实产品截图降低理解成本

## 视觉系统

继承 v1 设计：
- 底色：`#f8f6f1`（宣纸白）
- 主文字：`#1a1816`（浓墨）
- 次级文字：`#6b6560`（淡墨灰）
- 强调色：`#a83f2e`（朱砂）
- 标题字体：`LXGW WenKai` Regular
- 正文字体：系统无衬线栈
- 圆角：`2px`
- 纹理：全局宣纸噪点

新增：
- Navbar、Hero、Footer 加入项目 LOGO（`docs/images/logo.png`）

## 页面结构

共 8 个区块：

1. **Navbar** — 左侧 LOGO + 品牌名，右侧锚点，CTA 按钮
2. **Hero** — LOGO、主标题、副标题、一句话定位、双 CTA
3. **ValueProp** — 一句话核心价值
4. **BackstageFrontstage** — 左右对比：幕后工作室 vs 幕前写作台
5. **Genesis** — 创世四步流程
6. **Features** — 6 项真实产品能力，每项配截图
7. **QuickStart** — 三步上手
8. **DownloadCTA + Footer** — 下载号召、平台标签、版权信息

## 文案设计

### Hero

- 主标题：**把一句话创意，变成一本有序的小说**
- 副标题：草苔 StoryMoss 是专为长篇小说作者设计的系统工作台。幕后规划角色、场景、世界观；幕前沉浸式写作；Genesis 一键生成故事框架。
- 小字：分时介入，写得快也审得深；资产不崩，角色、伏笔、设定始终自洽。
- CTA：免费下载桌面版 / 看 Genesis 如何工作

### ValueProp

- **草苔是专为长篇小说作者设计的系统工作台：幕后管理故事资产，幕前沉浸式写作，Genesis 把创意变成可执行的创作结构。**

### BackstageFrontstage

- 标题：两个空间，各尽其职
- 左栏：幕后工作室
  - 管理故事、角色、场景、世界观
  - 知识图谱可视化关系
  - 伏笔看板追踪线索
  - AI 模型与提示词配置
- 右栏：幕前写作台
  - 极简、全屏、自动保存
  - 底部输入栏随时调用 AI
  - 续写、润色、改紧张感
  - 不打断心流

### Genesis

- 标题：从一句话创意，到可写的世界
- 副标题：输入一句话，30–90 秒生成故事框架
- 四步：
  1. 概念解析 — 一句话创意 → 题材画像
  2. 策略选择 — 雪花 / HDWB 等创作方法论
  3. 开篇骨架 — 主角目标、戏剧冲突、世界锚点
  4. 生成正文 — 自动进入幕前，可立即开始写作

### Features

6 项能力，每项配真实截图：
1. 故事与场景管理
2. 角色与世界观
3. 知识图谱与伏笔追踪
4. AI 续写 / 润色 / 改紧张感
5. 拆书与叙事分析
6. 提示词注册表与模型管理

### QuickStart

- 标题：三步开始写
- 步骤：
  1. 下载安装桌面版
  2. 用 Genesis 创建或手动新建故事
  3. 进入幕前写下第一段

### DownloadCTA

- 标题：开始你的第一本书
- 描述：Windows / macOS / Linux 桌面版免费下载。本地运行，数据归你。
- CTA：立即下载

## 组件调整

- `Navbar`：左侧增加 LOGO 图片
- `Hero`：顶部增加 LOGO，调整标题与副标题
- `ValueProp`：新增独立区块组件
- `BackstageFrontstage`：保留双栏结构，更新文案与截图
- `Genesis`：新增四步流程组件
- `Features`：从 4 项扩展为 6 项，更新截图与文案
- `TimeSliced`：移除独立区块，仅在 Hero 小字一句话带过
- `QuickStart`：保留三步流程
- `Footer`：增加 LOGO

## 截图需求

需要以下产品截图（来自 `docs/product-screenshots/`）：
- `01_dashboard.png` — 仪表盘/幕后入口
- `00_frontstage.png` — 幕前写作台
- `02_stories.png` — 故事与场景
- `03_characters.png` — 角色与世界观
- `06_knowledge-graph.png` — 知识图谱
- `11_foreshadowing.png` — 伏笔看板
- `09_book-deconstruction.png` — 拆书
- `07_skills.png` 或 `16_settings.png` — 提示词/模型
- `LOGO.png` — 品牌 LOGO

## 验收标准

- [ ] 视觉继承 v1 东方书卷方向
- [ ] LOGO 出现在 Navbar、Hero、Footer
- [ ] Hero 主标题、副标题、ValueProp 文案准确
- [ ] BackstageFrontstage 双栏文案来自项目文档
- [ ] Genesis 四步流程准确
- [ ] Features 6 项均配真实截图
- [ ] `npx vitest run` 通过
- [ ] `npx tsc --noEmit` 无错误
- [ ] `npm run build` 成功
- [ ] 部署到 ai.91z.net 后页面正常显示

## 后续步骤

1. 调用 `writing-plans` skill 生成实现计划
2. 按实现计划修改 `landing/` 代码
3. 运行构建、类型检查与测试
4. 重新部署到 ai.91z.net
