# StoryForge（草苔）官网落地页设计文档

> 创建日期: 2026-07-10
> 状态: 设计待评审
> 目标: 为 StoryForge 桌面应用设计一个面向新用户的下载转化落地页
> 受众: 小说作者、网络文学创作者、对 AI 辅助写作感兴趣的潜在用户

---

## 0. 方向锁定（用户已确认）

| 问题 | 答案 |
|---|---|
| 谁用、什么场景 | 面向新用户，引导下载桌面应用 |
| 美学方向 | 暖赭文学：暖赭石、旧纸张、深色墨迹 |
| 最记住的视觉元素 | 墨水扩散/书写动效 |
| 硬约束 | 复用项目现有技术栈：React + Vite + Tailwind CSS |
| 标志性微交互 | CTA 按钮按下时轻微缩放 + 墨水涟漪 |

---

## 1. 视觉主题与氛围

**视觉论点**：一张在书桌上缓缓展开的稿纸——暖赭文学质感，以羊皮纸为底、墨水为字、陶土色为点缀，让访问者第一眼就闻到纸张与咖啡的气息，而不是又一个“AI 工具”的冷感官网。

**氛围关键词**：温暖、沉浸、文学、手工感、专注。

**密度**：中等。 landing 页需要讲清楚产品和下载路径，但不能像应用后台那样信息密集。首屏留出大量呼吸空间，向下滚动时信息密度逐渐增加。

---

## 2. 色彩系统

使用 OKLCH 定义，便于在 Tailwind 中扩展。中性色向暖橙/赭色微微偏移，形成潜意识 cohesion。

| Token | OKLCH | 近似 HEX | 用途 |
|---|---|---|---|
| `--bg-parchment` | oklch(96% 0.01 85) | #f7f5ef | 页面主背景（羊皮纸） |
| `--bg-cream` | oklch(98% 0.008 85) | #fbf9f4 | 高亮区域、卡片背景 |
| `--bg-terracotta-soft` | oklch(92% 0.03 55) | #f3e9e3 | 强调区块背景（赭石浅色） |
| `--text-ink` | oklch(24% 0.01 65) | #2d2a26 | 主标题、正文 |
| `--text-charcoal` | oklch(42% 0.008 70) | #5a5550 | 次级文本、说明 |
| `--text-stone` | oklch(58% 0.006 75) | #827c75 | 辅助文本、元信息 |
| `--accent-terracotta` | oklch(55% 0.13 45) | #b85c3e | 主 CTA、强调色 |
| `--accent-terracotta-dark` | oklch(45% 0.12 45) | #8f442c | CTA hover/active |
| `--accent-gold` | oklch(72% 0.11 80) | #c9a35c | 高亮、小装饰、引用标记 |
| `--border-warm` | oklch(85% 0.01 80) | #ddd7cd | 分割线、卡片边框 |
| `--ink-wash` | oklch(35% 0.02 60 / 0.12) | — | 墨水涟漪、装饰晕染 |

**色彩比例**：60% 羊皮纸/奶油中性面，30% 墨灰文本与暖边框，10% 赭石+金色强调。

---

## 3. 字体规则

**拒绝默认字体**。本项目旧设计系统使用 Cinzel / Crimson Pro 作为 display，但根据设计规范，Crimson Pro 属于 reflex font，Landing 页不沿用。

| 层级 | 字体 | 备选 | 用途 |
|---|---|---|---|
| 中文 Display / 大标题 | 霞鹜文楷 (LXGW WenKai) | 思源宋体 (Source Han Serif CN) | Hero 主标题、章节号、品牌名 |
| 中文正文 | 思源宋体 (Source Han Serif CN) | Noto Serif SC, SimSun | 段落、说明、引用 |
| 英文 Display | Sorts Mill Goudy | Libre Baskerville, Georgia | 英文品牌名、装饰性英文 |
| 英文正文 | Libre Baskerville | Georgia, serif | 少量英文说明 |
| 标签 / 小字 | 系统无衬线 | — | 版本号、按钮小标签、下载元信息 |

**字号与字距**：

| 用途 | 桌面端 | 字重 | 行高 | 字距 |
|---|---|---|---|---|
| Hero 主标题 | 56–64px | 400 | 1.15 | -0.02em |
| 章节标题 | 36–40px | 500 | 1.2 | -0.015em |
| 小节标题 | 24px | 500 | 1.3 | -0.01em |
| 正文 | 18px | 400 | 1.75 | 0 |
| 小字/标签 | 14px | 400 | 1.5 | 0.02em |

**排版细节**：
- 标题使用 `text-wrap: balance`；正文使用 `text-wrap: pretty`。
- 引用块左侧不使用粗 border-left，而是使用金色短横线 + 大引号装饰。
- 中文正文避免两端对齐（会制造 rivers），使用左对齐。

---

## 4. 组件样式

### 主按钮（Primary CTA）

- 背景：`--accent-terracotta`
- 文字：`--bg-cream`，18px，weight 500
- 圆角：`8px`（统一命名半径 scale 中的 `md`）
- 内边距：`px-8 py-3.5`
- 阴影：`0 4px 14px oklch(55% 0.13 45 / 0.25)`
- Hover：背景过渡到 `--accent-terracotta-dark`，阴影加深
- Active：`scale(0.96)`，同时触发墨水涟漪（从点击点扩散的半透明墨斑）
- Focus：`ring-2 ring-offset-2 ring-terracotta`

### 次按钮（Secondary）

- 背景：透明
- 边框：`1.5px solid --accent-terracotta`
- 文字：`--accent-terracotta-dark`
- Hover：背景填充 `--bg-terracotta-soft`
- Active：`scale(0.97)`

### 导航栏

- 固定顶部，高度 72px
- 背景：`--bg-parchment` + 底部 `1px --border-warm` 分割线
- 滚动后增加轻微阴影：`0 1px 3px rgba(0,0,0,0.06)`
- 左侧：草苔 Logo + 品牌名
- 右侧：下载按钮（主按钮小尺寸）
- 移动端：汉堡菜单，展开后为全屏导航

### 引用块（Pull Quote）

- 无左侧粗线；使用顶部金色短横线（40px 宽，2px 高）
- 字体：霞鹜文楷 / Sorts Mill Goudy italic
- 颜色：`--text-ink` 85% 透明度

### 功能区块卡片

- 仅在内容需要聚合时使用卡片；默认不使用卡片装饰。
- 卡片背景：`--bg-cream`
- 圆角：12px
- 阴影：`0 2px 8px rgba(0,0,0,0.04)`
- Hover：阴影加深 + `translateY(-2px)`

---

## 5. 布局原则

**网格**：以单栏为主，最大宽度 `1100px`，居中对齐。部分 Feature 段落使用非对称两栏（文字 5/12，视觉 7/12 或反之），避免千篇一律的中心对称。

**间距系统**：

| Token | 值 | 用途 |
|---|---|---|
| `space-xs` | 8px | 行内小间隙 |
| `space-sm` | 16px | 组件内间距 |
| `space-md` | 32px | 小节间距 |
| `space-lg` | 64px | 区块内部间距 |
| `space-xl` | 120px | 区块之间（桌面） |

**区块节奏**：
1. Hero（全屏高度 100vh，内容垂直居中）
2. 痛点区（简短，左对齐大标题）
3. 解法区（双空间概念，图文交错）
4. 功能长卷（4 个 feature 段落，图文左右交替）
5. 分时介入架构（一个独立的深色/重色强调区，背景 `--bg-terracotta-soft`）
6. 下载 CTA 区（简洁，大图或纯文字聚焦）
7. Footer（极简，版权 + 下载链接）

---

## 6. 深度与层级

Landing 页为浅色模式。层级通过背景色阶 + 投影表达，不使用玻璃拟态作为默认卡片表面。

| 层级 | 实现 | 说明 |
|---|---|---|
| 页面画布 | `--bg-parchment` | 最底层 |
| 普通内容 | 无额外背景 | 直接铺在画布上 |
| 高亮/卡片 | `--bg-cream` + 轻微投影 | 比画布亮 2% 左右 |
| 强调区块 | `--bg-terracotta-soft` | 用于分时架构区 |
| 固定导航 | `--bg-parchment` + 底部 hairline | 滚动后出现阴影 |

投影规范：
- 小：`0 1px 3px rgba(0,0,0,0.06)`
- 中：`0 4px 14px rgba(0,0,0,0.08)`
- 大（CTA）：`0 8px 24px oklch(55% 0.13 45 / 0.2)`

---

## 7. 动效与交互

### 首屏加载

- Hero 主标题按词组/短语逐步显现，模拟笔尖落纸（`opacity 0→1`, `translateY(12px)→0`, `blur(4px)→0`）。
- 伴随轻微的墨水晕染装饰元素从 0 放大到 1。
- 时长：整体 1.2s， stagger ~120ms。
- 必须尊重 `prefers-reduced-motion`：禁用模糊与位移，仅保留 opacity 淡入。

### 滚动揭示

- 每个区块标题和内容拆分为语义块，进入视口时 stagger 淡入上滑。
- 使用 `transform` 和 `opacity` 动画，避免布局属性。
- Easing：`cubic-bezier(0.16, 1, 0.3, 1)`。

### CTA 墨水涟漪

- 点击主按钮时，从点击坐标生成一个径向渐变墨斑。
- 墨斑：`scale(0)→scale(4)`, `opacity 0.35→0`。
- 按钮同时 `scale(0.96)`。
- 使用 CSS 自定义属性 `--ripple-x`, `--ripple-y` 定位。

### 图片/截图

- 所有截图使用 `loading="lazy"`（首屏除外）。
- 截图外加 `outline: 1px solid rgba(0,0,0,0.08); outline-offset: -1px`，使其在浅色背景上hold住边界。

---

## 8. 响应式行为

| 断点 | 行为 |
|---|---|
| 默认（< 768px） | 单栏，字号整体降一档，Hero 标题 36–40px，区块间距 64px |
| `md`（≥ 768px） | 部分 feature 可开启两栏，导航完整展示 |
| `lg`（≥ 1024px） | 完整桌面布局，最大宽度 1100px，左右留白充足 |
| `xl`（≥ 1280px） | 保持 1100px 内容宽度，增加装饰性边距/边注 |

**移动端特别规则**：
- 导航折叠为汉堡菜单。
- Hero 副标题不超过 2 行。
- CTA 按钮宽度 100%，避免小点击区。
- 所有可点击目标最小 44×44px。

---

## 9. 内容大纲

### 9.1 Hero

- 品牌名：草苔 StoryForge
- 主标题：把混沌的长篇，写成有序的小说
- 副标题：AI 在需要时随行辅助。幕后管理故事、角色、场景、世界观；幕前沉浸式写作。
- 主 CTA：免费下载（桌面版）
- 次 CTA：查看功能 →
- 装饰： faint ink wash spot（半透墨斑），位于标题后方

### 9.2 痛点区

- 标题：写长篇，最怕的不是没灵感
- 三个痛点（不使用三列等宽卡片，而是用一段引言 + 三个边注/marginalia）：
  - 角色写着写着就崩了
  - 伏笔埋了却忘了回收
  - 设定越写越自相矛盾

### 9.3 解法区

- 标题：幕后规划，幕前写作
- 双栏布局：
  - 左：幕后工作室示意图/截图
  - 右：文字说明
- 下一节反转：
  - 左：文字
  - 右：幕前写作界面截图

### 9.4 功能长卷

四个 feature 段落，图文左右交替：

1. **故事与场景管理** — 把一本小说拆成可管理的故事、章节、场景。
2. **角色与世界观** — 系统化人设 + 知识图谱，避免 AI 吃书。
3. **AI 续写与润色** — 底部输入栏发指令，不打扰心流。
4. **拆书与分析** — 上传参考小说，学习经典结构。

### 9.5 分时介入架构（强调区）

- 背景：`--bg-terracotta-soft`
- 标题：写得快，也审得深
- 三段式说明：
  - 写作时刻：秒出正文
  - 审计时刻：后台自动审校，标注回流
  - 洞察时刻：叙事健康度报告

### 9.6 最终 CTA

- 标题：开始你的第一本书
- 副标题：Windows / macOS / Linux 桌面版免费下载
- 主按钮：立即下载
- 下方小字：开源项目，GitHub 上已有 X stars（可选）

### 9.7 Footer

- 极简：版权 © 2026 StoryForge Team
- 链接：GitHub、用户指南、更新日志

---

## 10. Do's and Don'ts

| Do | Don't |
|---|---|
| 使用暖赭、奶油、墨水组成的文学配色 | 使用紫色/蓝色渐变或“AI 感”霓虹色 |
| 让 Hero 标题具有手写/印刷感的字体气质 | 使用 Inter 或 Crimson Pro 作为默认展示字体 |
| 用图文交替、引言边注打破对称 | 做三列等宽 icon+heading+paragraph 的功能卡 |
| 用背景色阶和投影表达层级 | 默认给每个区块加 `border: 1px solid` 或玻璃拟态 |
| 让 CTA 按钮有墨水润开的反馈 | 使用 generic 的 hover 颜色变化 |
| 首屏一句话讲清价值：把混沌的长篇，写成有序的小说 | 首屏堆砌功能列表 |
| 所有动画尊重 `prefers-reduced-motion` | 默认播放复杂动画不考虑无障碍 |
| 移动端保证 CTA 全宽、点击区足够 | 在手机上保留小尺寸桌面导航和按钮 |

---

## 11. 技术实现要点

- **CSS 策略**：Tailwind CSS only。在 `tailwind.config.js` 中扩展颜色和字体。
- **动画库**：Framer Motion（项目已依赖），用于滚动揭示和 Hero 入场。注意不要把 Framer Motion 动画和 CSS transition 同时作用在同一元素的同一属性上。
- **图标**：lucide-react（项目已有），避免混用多套图标。
- **字体加载**：通过 Google Fonts / 中文网字计划 CDN 引入 LXGW WenKai 与 Source Han Serif CN，设置 `font-display: swap`。
- **涟漪效果**：纯 CSS + React onClick 设置自定义属性实现，不依赖第三方库。
- **静态资源**：截图使用 WebP，并提供 alt 文本；首屏主图可预加载。

---

## 12. Agent Prompt Guide

后续让其他 agent 实现时，可直接使用以下精确提示片段：

**Hero 区域**：
> 在 `bg-parchment` 背景上，创建一个居中的 Hero。主标题使用 `font-display`（霞鹜文楷 / Sorts Mill Goudy），56px，weight 400，行高 1.15，字距 -0.02em，颜色 `text-ink`。副标题 20px，`font-body`（思源宋体 / Libre Baskerville），颜色 `text-charcoal`，最大宽度 640px。主 CTA 使用 `bg-terracotta text-cream rounded-lg px-8 py-3.5 shadow-cta`，Hover 变 `bg-terracotta-dark`，Active `scale-96`。点击时从点击点触发墨水涟漪动画。Hero 标题加载时按词组 stagger 淡入上滑（opacity 0→1, translateY 12px→0, blur 4px→0，stagger 120ms，easing cubic-bezier(0.16,1,0.3,1)）。

**Feature 段落**：
> 创建一个图文交替的两栏区块，左栏文字，右栏图片。标题 36px `font-display text-ink`，正文 18px `font-body text-charcoal leading-relaxed`。图片带 subtle shadow 和 1px inset outline。进入视口时，标题先淡入，正文和图像 stagger 100ms 后淡入。

**分时介入强调区**：
> 一个全宽区块，背景 `bg-terracotta-soft`。标题居中，36px，颜色 `text-ink`。下方三个步骤横向排列（移动端堆叠），每个步骤上方有一个大号数字（01/02/03），使用 `font-display`，颜色 `accent-gold`。步骤说明 18px，颜色 `text-charcoal`。

**按钮涟漪组件**：
> 封装一个 `InkRippleButton`。按钮 relative overflow-hidden。点击时创建一个 absolute div，背景为径向渐变 `radial-gradient(circle, oklch(35% 0.02 60 / 0.35) 0%, transparent 70%)`，通过 CSS 自定义属性 `--ripple-x`、`--ripple-y` 定位，动画 `scale(0)→scale(4)` + `opacity(0.35)→0`，持续 600ms，easing ease-out。

---

## 13. 验收标准

- [ ] 首屏在 3 秒内明确传递产品价值。
- [ ] 品牌名“草苔”和“StoryForge”在首屏可见。
- [ ] CTA 按钮点击反馈（缩放 + 涟漪）可工作。
- [ ] 移动端 375px 宽度下布局不溢出、文本可读、CTA 可点击。
- [ ] 动画在 `prefers-reduced-motion: reduce` 下降级为简单淡入或不播放。
- [ ] 通过 Lighthouse 可访问性检查（无 contrast 错误，图片有 alt，按钮有 focus 状态）。
