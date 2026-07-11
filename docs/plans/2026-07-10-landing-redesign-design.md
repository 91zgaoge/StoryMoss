# StoryMoss 落地页重新设计文档

## 背景

StoryMoss（草苔）已有落地页实现位于 `landing/`，采用暖赭文学风格。用户反馈现有视觉「太普通」，希望转向更具辨识度的「极简东方书卷」美学，同时调整页面结构、加入真实产品截图与快速上手指南。

## 设计目标

- 建立与「AI 辅助小说创作」产品气质高度契合的视觉识别
- 通过真实截图降低用户理解成本
- 用「三步上手」降低首次使用门槛
- 保持首屏加载快、动画克制、可访问性良好

## 视觉系统

### 色彩

| Token | 值 | 用途 |
|-------|------|------|
| `parchment` | `#f8f6f1` | 页面底色，宣纸白 |
| `ink` | `#1a1816` | 主标题、正文重点 |
| `charcoal` | `#6b6560` | 次级文字、描述 |
| `cinnabar` | `#a83f2e` | CTA、hover、强调标签 |
| `ink-line` | `#e3ded4` | 边框、分隔线 |
| `ink-wash` | `rgba(26, 24, 22, 0.03)` | 背景晕染、水墨纹理 |

### 字体

- **标题**：`LXGW WenKai`（霞鹜文楷），仅加载 Regular（400）一个字重
- **正文**：系统无衬线栈 `system-ui, -apple-system, "PingFang SC", "Microsoft YaHei", sans-serif`
- **代码/标签**：`"SF Mono", "JetBrains Mono", monospace`

### 纹理与质感

- 全局叠加极淡宣纸噪点纹理，透明度 4%
- 区块分隔使用 1px 细线或大量留白，不用色块
- 圆角统一为 `2px`，接近直角，模拟裁纸

### 间距

- 内容最大宽度：`980px`
- 区块上下内边距：桌面 `160px`，移动端 `100px`
- 组件间距以 `8px` 为基准倍数

## 页面结构

共 8 个区块，按滚动顺序排列：

1. **Navbar** — 固定导航，含品牌、锚点、下载按钮、移动端抽屉菜单
2. **Hero** — 大字标题、副标题、双 CTA、产品主界面剪影
3. **PainPoints** — 三个痛点卡片：角色崩坏、伏笔遗忘、设定矛盾
4. **Solution** — 左右双栏：幕后工作室 vs 幕前写作台
5. **Features** — 四项功能长卷，每项配真实产品截图
6. **TimeSliced** — 分时介入三步骤：写作 / 审计 / 洞察
7. **QuickStart** — 新增：下载 → 创建故事 → 开始写作
8. **DownloadCTA + Footer** — 下载号召、平台标签、页脚链接

## 组件设计

### 可复用组件

- `InkButton`：直角按钮，Primary 朱砂底白字，Secondary 白底黑字，hover 背景微变
- `SectionTitle`：小字标签 + 大标题 + 副标题
- `FeatureFrame`：装裱截图的细边框容器，hover 边框变朱砂
- `StepCard`：编号 + 标题 + 描述
- `PainCard`：痛点卡片，hover 底部细线变朱砂
- `SplitPane`：Solution 双栏，中间竖线分隔

### 交互细节

- 锚点平滑滚动
- 移动端菜单全屏淡入
- 按钮 hover 仅颜色过渡，无位移/缩放
- 截图容器 hover 边框颜色过渡 200ms
- 无弹窗、轮播、自动播放

## 动效与可访问性

### 动效原则

- 仅使用淡入 + 轻微上移
- 所有滚动触发只执行一次
- 通过 `useReducedMotion` 响应 `prefers-reduced-motion`，关闭全部动画

### 具体动效

- Hero 标题逐行淡入，间隔 0.1s
- 区块标题进入视口时整体淡入上移
- 卡片/步骤 stagger 淡入，间隔 0.08s

### 可访问性

- 所有交互元素有 `:focus-visible` 状态
- 图片提供有意义的 `alt` 文本
- 颜色对比度符合 WCAG AA
- 语义化 HTML 结构
- 按钮使用真实 `<button>` 或 `<a>`

## 技术实现

### 技术栈

- React 18 + Vite 6 + TypeScript 5.8
- Tailwind CSS 3
- Framer Motion（仅用于淡入动画）
- `lucide-react` 图标

### 性能目标

- 字体只加载一个字重，使用 `font-display: swap`
- 产品截图使用 WebP/AVIF 并懒加载
- 水墨质感用 CSS 实现，不用大图片
- 构建产物目标：CSS < 50KB，JS < 150KB

### 文件结构

```
landing/src/
  App.tsx
  main.tsx
  index.css
  components/
    Navbar.tsx
    Hero.tsx
    PainPoints.tsx
    Solution.tsx
    Features.tsx
    TimeSliced.tsx
    QuickStart.tsx
    DownloadCTA.tsx
    Footer.tsx
    InkButton.tsx
    SectionTitle.tsx
    FeatureFrame.tsx
    StepCard.tsx
  hooks/
    useReducedMotion.ts
  test/
    setup.ts
    components/
      InkButton.test.tsx
      Navbar.test.tsx
      QuickStart.test.tsx
```

### 测试

- Vitest + Testing Library
- 覆盖 `InkButton`、`Navbar`、`QuickStart`
- 保持 `npx vitest run` 全绿

## 验收标准

- [ ] 视觉符合「极简东方书卷」方向
- [ ] 8 个区块完整呈现，文案准确
- [ ] Features 使用真实产品截图
- [ ] 动画在 `prefers-reduced-motion` 下完全关闭
- [ ] 移动端响应式正常
- [ ] `npm run build` 成功
- [ ] `npx vitest run` 通过
- [ ] `npx tsc --noEmit` 无错误

## 后续步骤

1. 调用 `writing-plans` skill 生成实现计划
2. 按实现计划重写 `landing/` 代码
3. 运行构建、类型检查与测试
4. 更新 README / CHANGELOG 相关条目
