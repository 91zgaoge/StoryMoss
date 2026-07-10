# StoryForge 落地页 v2 重新设计实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在保留现有「极简东方书卷」视觉基础上，全面重写 StoryForge 落地页内容与结构：加入 LOGO、突出「幕后+幕前双空间」与「Genesis 创世」两大特色、扩展 Features 至 6 项、移除 TimeSliced 独立区块，并重新部署到 ai.91z.net。

**Architecture:** 复用现有 React + Vite + Tailwind + Framer Motion 组件体系，新增/修改页面区块组件；通过真实产品截图与项目 LOGO 增强产品介绍；最终通过 `npm run deploy` 脚本重新部署。

**Tech Stack:** React 18, Vite 6, TypeScript 5.8, Tailwind CSS 3, Framer Motion, lucide-react, basic-ftp

## Global Constraints

- 项目根目录：`/Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page`
- 落地页目录：`landing/`
- 所有命令默认在 `landing/` 下执行，部署命令除外
- 颜色、字体、圆角等视觉 token 与 v1 保持一致
- 必须响应 `prefers-reduced-motion`
- 每次任务完成后必须提交
- 最终必须通过 `npx vitest run`、`npm run build`、`npx tsc --noEmit`
- 部署到 ai.91z.net 需使用 `npm run deploy`（FTP 凭据已通过环境变量传入）

---

## File Structure

```
landing/
  public/
    logo.png                 # 从项目根 docs/images/logo.png 复制
    screenshots/             # 已有 4 张，新增仪表盘、知识图谱、伏笔看板、设置截图
  src/
    App.tsx                  # 组装 8 个区块，移除 TimeSliced
    components/
      Navbar.tsx             # 左侧增加 LOGO
      Hero.tsx               # 增加 LOGO，重写标题/副标题
      ValueProp.tsx          # 新增一句话定位组件
      BackstageFrontstage.tsx # 新增（或复用 Solution.tsx 改名），双空间对比
      Genesis.tsx            # 新增创世四步流程
      Features.tsx           # 扩展为 6 项，更新截图与文案
      QuickStart.tsx         # 保留，文案微调到与 Genesis 衔接
      DownloadCTA.tsx        # 保留
      Footer.tsx             # 增加 LOGO
      InkButton.tsx          # 不变
      SectionTitle.tsx       # 不变
      FeatureFrame.tsx       # 不变
      StepCard.tsx           # 不变
```

---

### Task 1: 复制 LOGO 与新增截图到 public/

**Files:**
- Create: `landing/public/logo.png`
- Create: `landing/public/screenshots/dashboard.png`
- Create: `landing/public/screenshots/knowledge-graph.png`
- Create: `landing/public/screenshots/foreshadowing.png`
- Create: `landing/public/screenshots/settings.png`

**Interfaces:**
- Produces: 静态资源文件，供组件通过 `/logo.png` 和 `/screenshots/*.png` 引用

- [ ] **Step 1: 复制 LOGO**

Run:
```bash
cp /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/docs/images/logo.png /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/public/logo.png
```

- [ ] **Step 2: 复制新增截图**

Run:
```bash
mkdir -p /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/public/screenshots
cp /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/docs/product-screenshots/01_dashboard.png /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/public/screenshots/dashboard.png
cp /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/docs/product-screenshots/06_knowledge-graph.png /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/public/screenshots/knowledge-graph.png
cp /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/docs/product-screenshots/11_foreshadowing.png /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/public/screenshots/foreshadowing.png
cp /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/docs/product-screenshots/16_settings.png /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/public/screenshots/settings.png
```

- [ ] **Step 3: 验证文件存在**

Run:
```bash
ls -la /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/public/logo.png /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/public/screenshots/
```

Expected: `logo.png` 与 8 张截图均存在。

- [ ] **Step 4: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/public/logo.png landing/public/screenshots/
git commit -m "assets(landing): add logo and additional product screenshots"
```

---

### Task 2: 更新 Navbar 加入 LOGO

**Files:**
- Modify: `landing/src/components/Navbar.tsx`
- Modify: `landing/src/components/__tests__/Navbar.test.tsx`

**Interfaces:**
- Consumes: `InkButton`
- Produces: Navbar 左侧显示 LOGO 图片 + 品牌名

- [ ] **Step 1: 更新测试断言**

编辑 `landing/src/components/__tests__/Navbar.test.tsx`：

```tsx
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect } from 'vitest';
import { Navbar } from '../Navbar';

describe('Navbar', () => {
  it('renders logo and brand', () => {
    render(<Navbar />);
    expect(screen.getByAltText('StoryForge 草苔')).toBeInTheDocument();
    expect(screen.getByText('草苔')).toBeInTheDocument();
    expect(screen.getByText('StoryForge')).toBeInTheDocument();
  });

  it('renders links on desktop', () => {
    render(<Navbar />);
    expect(screen.getByRole('link', { name: /功能/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /免费下载/i })).toBeInTheDocument();
  });

  it('toggles mobile menu', async () => {
    render(<Navbar />);
    const toggle = screen.getByLabelText(/打开菜单/i);
    await userEvent.click(toggle);
    expect(screen.getByLabelText(/关闭菜单/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/Navbar.test.tsx
```

Expected: FAIL — `StoryForge 草苔` alt text not found.

- [ ] **Step 3: 重写 Navbar 组件**

编辑 `landing/src/components/Navbar.tsx`：

```tsx
import { useState, useEffect } from 'react';
import { Menu, X } from 'lucide-react';
import { InkButton } from './InkButton';

export function Navbar() {
  const [scrolled, setScrolled] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 8);
    window.addEventListener('scroll', onScroll);
    return () => window.removeEventListener('scroll', onScroll);
  }, []);

  const links = [
    { href: '#features', label: '功能' },
    { href: '#approach', label: '方法' },
    { href: '#genesis', label: '创世' },
    { href: '#quickstart', label: '上手' },
    { href: '#download', label: '下载' },
  ];

  return (
    <header
      className={`fixed top-0 left-0 right-0 z-50 h-[72px] border-b border-ink-line bg-parchment/95 backdrop-blur-sm transition-shadow duration-200 ${
        scrolled ? 'shadow-nav' : ''
      }`}
    >
      <nav
        className="mx-auto flex h-full max-w-[980px] items-center justify-between px-6"
        aria-label="主导航"
      >
        <a href="/" className="flex items-center gap-2.5 font-display text-xl text-ink">
          <img
            src="/logo.png"
            alt="StoryForge 草苔"
            className="h-8 w-8 object-contain"
          />
          <span>草苔</span>
          <span className="font-body text-xs tracking-wide text-charcoal">StoryForge</span>
        </a>

        <div className="hidden items-center gap-8 md:flex">
          {links.map((l) => (
            <a
              key={l.href}
              href={l.href}
              className="text-sm text-charcoal transition-colors hover:text-ink"
            >
              {l.label}
            </a>
          ))}
          <InkButton variant="primary" className="px-5 py-2 text-xs">
            免费下载
          </InkButton>
        </div>

        <button
          className="flex h-10 w-10 items-center justify-center rounded-[2px] text-ink md:hidden"
          aria-label={mobileOpen ? '关闭菜单' : '打开菜单'}
          aria-expanded={mobileOpen}
          onClick={() => setMobileOpen((s) => !s)}
        >
          {mobileOpen ? <X size={22} /> : <Menu size={22} />}
        </button>
      </nav>

      {mobileOpen && (
        <div className="absolute left-0 right-0 top-[72px] border-b border-ink-line bg-parchment px-6 py-5 md:hidden">
          <ul className="flex flex-col gap-4">
            {links.map((l) => (
              <li key={l.href}>
                <a
                  href={l.href}
                  className="block text-charcoal hover:text-ink"
                  onClick={() => setMobileOpen(false)}
                >
                  {l.label}
                </a>
              </li>
            ))}
            <li>
              <InkButton variant="primary" className="w-full">
                免费下载
              </InkButton>
            </li>
          </ul>
        </div>
      )}
    </header>
  );
}
```

- [ ] **Step 4: 运行测试确认通过**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/Navbar.test.tsx
```

Expected: PASS (3 tests).

- [ ] **Step 5: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/Navbar.tsx landing/src/components/__tests__/Navbar.test.tsx
git commit -m "feat(landing): add logo to Navbar"
```

---

### Task 3: 重写 Hero 加入 LOGO 与新文案

**Files:**
- Modify: `landing/src/components/Hero.tsx`

**Interfaces:**
- Consumes: `InkButton`, `useReducedMotion`
- Produces: 新 Hero 文案与 LOGO

- [ ] **Step 1: 重写 Hero**

编辑 `landing/src/components/Hero.tsx`：

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { InkButton } from './InkButton';
import { ChevronDown } from 'lucide-react';

export function Hero() {
  const reduced = useReducedMotion();

  const container = {
    hidden: {},
    visible: { transition: { staggerChildren: 0.1 } },
  };

  const child = {
    hidden: { opacity: 0, y: 16 },
    visible: {
      opacity: 1,
      y: 0,
      transition: { duration: 0.6, ease: [0.16, 1, 0.3, 1] as const },
    },
  };

  return (
    <section className="relative flex min-h-screen flex-col items-center justify-center px-6 pt-[72px] text-center">
      <div className="absolute inset-0 -z-10 overflow-hidden">
        <div className="paper-texture absolute inset-0" />
        <div className="absolute left-1/2 top-1/2 h-[520px] w-[520px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-ink-wash blur-3xl" />
      </div>

      <motion.div
        variants={reduced ? undefined : container}
        initial="hidden"
        animate="visible"
        className="max-w-[720px]"
      >
        <motion.div variants={reduced ? undefined : child} className="mb-6 flex justify-center">
          <img
            src="/logo.png"
            alt="StoryForge 草苔"
            className="h-16 w-16 object-contain"
          />
        </motion.div>

        <motion.h1
          variants={reduced ? undefined : child}
          className="mb-6 text-[40px] leading-[1.12] tracking-[-0.02em] text-ink md:text-[64px]"
        >
          把一句话创意，<br className="hidden md:block" />
          变成一本有序的小说
        </motion.h1>

        <motion.p
          variants={reduced ? undefined : child}
          className="mx-auto mb-4 max-w-[600px] text-base leading-relaxed text-charcoal md:text-lg"
        >
          草苔 StoryForge 是专为长篇小说作者设计的系统工作台。幕后规划角色、场景、世界观；幕前沉浸式写作；Genesis 一键生成故事框架。
        </motion.p>

        <motion.p
          variants={reduced ? undefined : child}
          className="mx-auto mb-10 max-w-[600px] text-sm text-stone"
        >
          分时介入，写得快也审得深；资产不崩，角色、伏笔、设定始终自洽。
        </motion.p>

        <motion.div
          variants={reduced ? undefined : child}
          className="flex flex-col items-center justify-center gap-4 sm:flex-row"
        >
          <InkButton variant="primary">免费下载桌面版</InkButton>
          <a href="#genesis">
            <InkButton variant="secondary" className="group">
              看 Genesis 如何工作
              <ChevronDown
                className="ml-1 inline-block transition-transform group-hover:translate-y-0.5"
                size={16}
              />
            </InkButton>
          </a>
        </motion.div>
      </motion.div>
    </section>
  );
}
```

- [ ] **Step 2: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/Hero.tsx
git commit -m "feat(landing): rewrite Hero with logo and new positioning"
```

---

### Task 4: 新增 ValueProp 组件

**Files:**
- Create: `landing/src/components/ValueProp.tsx`
- Create: `landing/src/components/__tests__/ValueProp.test.tsx`

**Interfaces:**
- Consumes: `useReducedMotion`
- Produces: `<ValueProp>`

- [ ] **Step 1: 写测试**

创建 `landing/src/components/__tests__/ValueProp.test.tsx`：

```tsx
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { ValueProp } from '../ValueProp';

describe('ValueProp', () => {
  it('renders the value proposition', () => {
    render(<ValueProp />);
    expect(
      screen.getByText(/草苔是专为长篇小说作者设计的系统工作台/i)
    ).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/ValueProp.test.tsx
```

Expected: FAIL — `ValueProp` not found.

- [ ] **Step 3: 实现组件**

创建 `landing/src/components/ValueProp.tsx`：

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';

export function ValueProp() {
  const reduced = useReducedMotion();

  return (
    <section className="mx-auto max-w-[980px] px-6 pb-[100px] text-center md:pb-[160px]">
      <motion.p
        initial={reduced ? undefined : { opacity: 0, y: 16 }}
        whileInView={reduced ? undefined : { opacity: 1, y: 0 }}
        viewport={{ once: true, margin: '-100px' }}
        transition={{ duration: 0.6, ease: [0.16, 1, 0.3, 1] }}
        className="mx-auto max-w-[860px] text-[20px] leading-relaxed text-ink md:text-[26px]"
      >
        草苔是专为长篇小说作者设计的系统工作台：幕后管理故事资产，幕前沉浸式写作，Genesis 把创意变成可执行的创作结构。
      </motion.p>
    </section>
  );
}
```

- [ ] **Step 4: 运行测试确认通过**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/ValueProp.test.tsx
```

Expected: PASS (1 test).

- [ ] **Step 5: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/ValueProp.tsx landing/src/components/__tests__/ValueProp.test.tsx
git commit -m "feat(landing): add ValueProp section"
```

---

### Task 5: 重写 BackstageFrontstage（双空间对比）

**Files:**
- Delete: `landing/src/components/Solution.tsx`
- Create: `landing/src/components/BackstageFrontstage.tsx`

**Interfaces:**
- Consumes: `SectionTitle`, `FeatureFrame`, `useReducedMotion`
- Produces: `<BackstageFrontstage>`

- [ ] **Step 1: 删除旧 Solution 组件**

Run:
```bash
rm /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/src/components/Solution.tsx
```

- [ ] **Step 2: 实现新组件**

创建 `landing/src/components/BackstageFrontstage.tsx`：

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { SectionTitle } from './SectionTitle';
import { FeatureFrame } from './FeatureFrame';

export function BackstageFrontstage() {
  const reduced = useReducedMotion();

  const item = {
    hidden: { opacity: 0, y: 20 },
    visible: {
      opacity: 1,
      y: 0,
      transition: { duration: 0.6, ease: [0.16, 1, 0.3, 1] as const },
    },
  };

  return (
    <section id="approach" className="border-y border-ink-line bg-cream py-[100px] md:py-[160px]">
      <div className="mx-auto max-w-[980px] px-6">
        <SectionTitle
          label="01"
          title="两个空间，各尽其职"
          description="把规划与写作拆成两个空间：幕后用 AI 系统化管好创作资产，幕前只留你和文字。"
        />

        <div className="space-y-16 md:space-y-24">
          <motion.div
            initial={reduced ? undefined : 'hidden'}
            whileInView={reduced ? undefined : 'visible'}
            viewport={{ once: true, margin: '-100px' }}
            variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
            className="grid items-center gap-10 md:grid-cols-2"
          >
            <motion.div variants={reduced ? undefined : item}>
              <h3 className="mb-4 text-2xl text-ink md:text-3xl">幕后工作室</h3>
              <ul className="space-y-3 text-charcoal">
                <li>管理故事、角色、场景、世界观</li>
                <li>知识图谱可视化人物、地点、事件关系</li>
                <li>伏笔看板追踪线索的埋下与回收</li>
                <li>AI 模型、提示词、创作方法论配置</li>
              </ul>
            </motion.div>
            <motion.div variants={reduced ? undefined : item}>
              <FeatureFrame src="/screenshots/dashboard.png" alt="幕后工作室仪表盘" />
            </motion.div>
          </motion.div>

          <motion.div
            initial={reduced ? undefined : 'hidden'}
            whileInView={reduced ? undefined : 'visible'}
            viewport={{ once: true, margin: '-100px' }}
            variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
            className="grid items-center gap-10 md:grid-cols-2"
          >
            <motion.div variants={reduced ? undefined : item} className="md:order-2">
              <h3 className="mb-4 text-2xl text-ink md:text-3xl">幕前写作台</h3>
              <ul className="space-y-3 text-charcoal">
                <li>极简、全屏、自动保存，无干扰码字环境</li>
                <li>底部输入栏随时调用 AI 续写、润色、改紧张感</li>
                <li>文思模式切换 AI 介入程度，被动或主动辅助</li>
                <li>创作主权在你，AI 只在需要时随行</li>
              </ul>
            </motion.div>
            <motion.div variants={reduced ? undefined : item} className="md:order-1">
              <FeatureFrame src="/screenshots/frontstage.png" alt="幕前沉浸式写作台" />
            </motion.div>
          </motion.div>
        </div>
      </div>
    </section>
  );
}
```

- [ ] **Step 3: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/Solution.tsx landing/src/components/BackstageFrontstage.tsx
git commit -m "feat(landing): replace Solution with BackstageFrontstage"
```

---

### Task 6: 新增 Genesis 创世流程组件

**Files:**
- Create: `landing/src/components/Genesis.tsx`
- Create: `landing/src/components/__tests__/Genesis.test.tsx`

**Interfaces:**
- Consumes: `SectionTitle`, `StepCard`, `useReducedMotion`
- Produces: `<Genesis>`

- [ ] **Step 1: 写测试**

创建 `landing/src/components/__tests__/Genesis.test.tsx`：

```tsx
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { Genesis } from '../Genesis';

describe('Genesis', () => {
  it('renders four genesis steps', () => {
    render(<Genesis />);
    expect(screen.getByText('概念解析')).toBeInTheDocument();
    expect(screen.getByText('策略选择')).toBeInTheDocument();
    expect(screen.getByText('开篇骨架')).toBeInTheDocument();
    expect(screen.getByText('生成正文')).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/Genesis.test.tsx
```

Expected: FAIL — `Genesis` not found.

- [ ] **Step 3: 实现组件**

创建 `landing/src/components/Genesis.tsx`：

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { SectionTitle } from './SectionTitle';
import { StepCard } from './StepCard';

const steps = [
  {
    number: '01',
    title: '概念解析',
    description: '把一句话创意解析成题材画像、核心冲突与世界锚点。',
  },
  {
    number: '02',
    title: '策略选择',
    description: '匹配雪花法、高密度世界构建等创作方法论，作为生成骨架。',
  },
  {
    number: '03',
    title: '开篇骨架',
    description: '生成主角目标、戏剧冲突与世界锚点，为正文铺设稳定结构。',
  },
  {
    number: '04',
    title: '生成正文',
    description: '自动进入幕前写作台，第一章正文已就绪，可立即开始修改与续写。',
  },
];

export function Genesis() {
  const reduced = useReducedMotion();

  const item = {
    hidden: { opacity: 0, y: 20 },
    visible: {
      opacity: 1,
      y: 0,
      transition: { duration: 0.6, ease: [0.16, 1, 0.3, 1] as const },
    },
  };

  return (
    <section id="genesis" className="mx-auto max-w-[980px] px-6 py-[100px] md:py-[160px]">
      <SectionTitle
        label="02"
        title="从一句话创意，到可写的世界"
        description="输入一句话，30–90 秒生成故事框架。Genesis 把灵感变成可执行的创作结构。"
      />

      <motion.div
        initial={reduced ? undefined : 'hidden'}
        whileInView={reduced ? undefined : 'visible'}
        viewport={{ once: true, margin: '-100px' }}
        variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
        className="grid gap-8 md:grid-cols-2 lg:grid-cols-4"
      >
        {steps.map((s) => (
          <motion.div key={s.number} variants={reduced ? undefined : item}>
            <StepCard number={s.number} title={s.title} description={s.description} />
          </motion.div>
        ))}
      </motion.div>
    </section>
  );
}
```

- [ ] **Step 4: 运行测试确认通过**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/Genesis.test.tsx
```

Expected: PASS (1 test).

- [ ] **Step 5: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/Genesis.tsx landing/src/components/__tests__/Genesis.test.tsx
git commit -m "feat(landing): add Genesis workflow section"
```

---

### Task 7: 扩展 Features 至 6 项

**Files:**
- Modify: `landing/src/components/Features.tsx`

**Interfaces:**
- Consumes: `SectionTitle`, `FeatureFrame`, `useReducedMotion`
- Produces: 6 项功能展示

- [ ] **Step 1: 重写 Features**

编辑 `landing/src/components/Features.tsx`：

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { SectionTitle } from './SectionTitle';
import { FeatureFrame } from './FeatureFrame';

const features = [
  {
    title: '故事与场景管理',
    description:
      '把一本小说拆成可管理的故事、章节、场景。每个场景都有出场角色、地点、状态，降低“写一章”的心理压力。',
    image: '/screenshots/stories.png',
    alt: '故事与场景管理界面',
  },
  {
    title: '角色与世界观',
    description:
      '系统化人设、关系网络、世界观设定。AI 在续写时严格遵循设定，避免“角色崩坏”和“吃书”。',
    image: '/screenshots/characters.png',
    alt: '角色与世界观管理界面',
  },
  {
    title: '知识图谱与伏笔追踪',
    description:
      '把角色、地点、事件、势力变成可交互网络；伏笔看板追踪每条线索的埋下与回收，防止烂尾。',
    image: '/screenshots/knowledge-graph.png',
    alt: '知识图谱界面',
  },
  {
    title: 'AI 续写与润色',
    description:
      '底部输入栏发指令：“续写下一段”“改得更紧张”“加入意外转折”。AI 随行辅助，但创作主权始终在你。',
    image: '/screenshots/frontstage.png',
    alt: '幕前 AI 续写界面',
  },
  {
    title: '拆书与叙事分析',
    description:
      '上传参考小说，AI 自动分析整体结构、章节节奏、角色出场频率，把“凭感觉写”变成“有参照地写”。',
    image: '/screenshots/book-deconstruction.png',
    alt: '拆书与叙事分析界面',
  },
  {
    title: '提示词注册表与模型管理',
    description:
      '35+ 个内置提示词统一注册、分类浏览、实时搜索、本地编辑覆盖；模型管理支持多模型配置与角色分配。',
    image: '/screenshots/settings.png',
    alt: '提示词注册表与模型管理界面',
  },
];

export function Features() {
  const reduced = useReducedMotion();

  const item = {
    hidden: { opacity: 0, y: 20 },
    visible: {
      opacity: 1,
      y: 0,
      transition: { duration: 0.6, ease: [0.16, 1, 0.3, 1] as const },
    },
  };

  return (
    <section id="features" className="border-y border-ink-line bg-cream py-[100px] md:py-[160px]">
      <div className="mx-auto max-w-[980px] px-6">
        <SectionTitle
          label="03"
          title="一套完整的创作系统"
          description="从灵感、规划、写作到分析，草苔把长篇小说创作的每个环节都装进了工作台。"
        />

        <div className="space-y-20 md:space-y-28">
          {features.map((f, idx) => {
            const isReversed = idx % 2 === 1;

            return (
              <motion.div
                key={f.title}
                initial={reduced ? undefined : 'hidden'}
                whileInView={reduced ? undefined : 'visible'}
                viewport={{ once: true, margin: '-100px' }}
                variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
                className="grid items-center gap-10 md:grid-cols-2"
              >
                <motion.div
                  variants={reduced ? undefined : item}
                  className={isReversed ? 'md:order-2' : ''}
                >
                  <h3 className="mb-3 text-2xl text-ink md:text-3xl">{f.title}</h3>
                  <p className="max-w-[480px] text-base leading-relaxed text-charcoal md:text-lg">
                    {f.description}
                  </p>
                </motion.div>

                <motion.div
                  variants={reduced ? undefined : item}
                  className={isReversed ? 'md:order-1' : ''}
                >
                  <FeatureFrame src={f.image} alt={f.alt} />
                </motion.div>
              </motion.div>
            );
          })}
        </div>
      </div>
    </section>
  );
}
```

- [ ] **Step 2: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/Features.tsx
git commit -m "feat(landing): expand Features to 6 items with new screenshots"
```

---

### Task 8: 更新 QuickStart 文案

**Files:**
- Modify: `landing/src/components/QuickStart.tsx`

**Interfaces:**
- Consumes: `SectionTitle`, `StepCard`, `useReducedMotion`
- Produces: 更新后的 QuickStart

- [ ] **Step 1: 更新文案**

编辑 `landing/src/components/QuickStart.tsx`：

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { SectionTitle } from './SectionTitle';
import { StepCard } from './StepCard';

const steps = [
  {
    number: '01',
    title: '下载安装桌面版',
    description: 'Windows / macOS / Linux 均可运行，本地使用，数据归你。',
  },
  {
    number: '02',
    title: '用 Genesis 创建故事',
    description: '输入一句话创意，30–90 秒生成故事框架、角色与开篇场景。',
  },
  {
    number: '03',
    title: '进入幕前写下第一段',
    description: '打开沉浸式写作界面，卡壳时随时呼叫 AI 续写或润色。',
  },
];

export function QuickStart() {
  const reduced = useReducedMotion();

  const item = {
    hidden: { opacity: 0, y: 20 },
    visible: {
      opacity: 1,
      y: 0,
      transition: { duration: 0.6, ease: [0.16, 1, 0.3, 1] as const },
    },
  };

  return (
    <section id="quickstart" className="mx-auto max-w-[980px] px-6 py-[100px] md:py-[160px]">
      <SectionTitle
        label="04"
        title="三步开始写"
        description="不需要复杂配置，安装后即可开始你的第一本书。"
      />

      <motion.div
        initial={reduced ? undefined : 'hidden'}
        whileInView={reduced ? undefined : 'visible'}
        viewport={{ once: true, margin: '-100px' }}
        variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
        className="grid gap-10 md:grid-cols-3"
      >
        {steps.map((s) => (
          <motion.div key={s.number} variants={reduced ? undefined : item}>
            <StepCard number={s.number} title={s.title} description={s.description} />
          </motion.div>
        ))}
      </motion.div>
    </section>
  );
}
```

- [ ] **Step 2: 运行测试确认通过**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/QuickStart.test.tsx
```

Expected: PASS (1 test)。

- [ ] **Step 3: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/QuickStart.tsx
git commit -m "feat(landing): update QuickStart copy"
```

---

### Task 9: 更新 Footer 加入 LOGO

**Files:**
- Modify: `landing/src/components/Footer.tsx`

**Interfaces:**
- Produces: 带 LOGO 的 Footer

- [ ] **Step 1: 重写 Footer**

编辑 `landing/src/components/Footer.tsx`：

```tsx
export function Footer() {
  return (
    <footer className="border-t border-ink-line bg-parchment py-10">
      <div className="mx-auto flex max-w-[980px] flex-col items-center justify-between gap-6 px-6 md:flex-row">
        <div className="flex items-center gap-2.5">
          <img
            src="/logo.png"
            alt="StoryForge 草苔"
            className="h-7 w-7 object-contain"
          />
          <span className="font-display text-lg text-ink">草苔</span>
          <span className="font-body text-xs tracking-wide text-charcoal">StoryForge</span>
        </div>

        <p className="text-sm text-charcoal">© 2026 StoryForge · 草苔</p>

        <div className="flex gap-6 text-sm text-charcoal">
          <a
            href="https://github.com/91zgaoge/StoryForge"
            className="hover:text-ink"
            target="_blank"
            rel="noreferrer"
          >
            GitHub
          </a>
          <a href="#" className="hover:text-ink">
            用户指南
          </a>
        </div>
      </div>
    </footer>
  );
}
```

- [ ] **Step 2: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/Footer.tsx
git commit -m "feat(landing): add logo to Footer"
```

---

### Task 10: 更新 App.tsx 组装新结构

**Files:**
- Modify: `landing/src/App.tsx`
- Delete: `landing/src/components/TimeSliced.tsx`

**Interfaces:**
- Consumes: 所有区块组件
- Produces: 新页面结构

- [ ] **Step 1: 删除 TimeSliced 组件**

Run:
```bash
rm /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/src/components/TimeSliced.tsx
```

- [ ] **Step 2: 重写 App.tsx**

编辑 `landing/src/App.tsx`：

```tsx
import { Navbar } from './components/Navbar';
import { Hero } from './components/Hero';
import { ValueProp } from './components/ValueProp';
import { BackstageFrontstage } from './components/BackstageFrontstage';
import { Genesis } from './components/Genesis';
import { Features } from './components/Features';
import { QuickStart } from './components/QuickStart';
import { DownloadCTA } from './components/DownloadCTA';
import { Footer } from './components/Footer';

export default function App() {
  return (
    <>
      <Navbar />
      <main>
        <Hero />
        <ValueProp />
        <BackstageFrontstage />
        <Genesis />
        <Features />
        <QuickStart />
        <DownloadCTA />
      </main>
      <Footer />
    </>
  );
}
```

- [ ] **Step 3: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/App.tsx landing/src/components/TimeSliced.tsx
git commit -m "feat(landing): assemble v2 landing page structure"
```

---

### Task 11: 全局验证

**Files:**
- 无

**Interfaces:**
- 无

- [ ] **Step 1: 运行类型检查**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx tsc --noEmit
```

Expected: 无错误。

- [ ] **Step 2: 运行测试**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run
```

Expected: PASS（至少 8 个测试：InkButton 3 + Navbar 3 + QuickStart 1 + Genesis 1 + ValueProp 1）。

- [ ] **Step 3: 运行构建**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npm run build
```

Expected: 构建成功。

- [ ] **Step 4: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/dist 2>/dev/null || true
git commit -m "chore(landing): verify build, types and tests for v2"
```

---

### Task 12: 重新部署到 ai.91z.net

**Files:**
- 无

**Interfaces:**
- 使用 `landing/scripts/deploy.js`

- [ ] **Step 1: 运行部署脚本**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
FTP_HOST=23.106.154.76 FTP_PORT=14121 FTP_USER=gaoge FTP_PASS=88152353 npm run deploy
```

Expected: 构建成功，上传 10+ 个文件（含 LOGO 与新截图）。

- [ ] **Step 2: 验证线上页面**

Run:
```bash
curl -s http://ai.91z.net/ | grep -E "(把一句话创意|两个空间|Genesis|草苔)" | head -5
curl -I http://ai.91z.net/assets/index-*.js | head -1
curl -I http://ai.91z.net/logo.png | head -1
```

Expected: HTML 包含新文案；JS/CSS/LOGO 返回 200。

---

## Self-Review

### Spec Coverage

| 设计文档要求 | 对应任务 |
|-------------|---------|
| 加入 LOGO | Task 1, Task 2, Task 3, Task 9 |
| 重写 Hero 文案 | Task 3 |
| 新增 ValueProp | Task 4 |
| 双空间 BackstageFrontstage | Task 5 |
| Genesis 四步流程 | Task 6 |
| Features 扩展为 6 项 | Task 7 |
| 移除 TimeSliced 独立区块 | Task 10 |
| QuickStart 更新 | Task 8 |
| 部署到 ai.91z.net | Task 12 |

### Placeholder Scan

- 无 "TBD", "TODO", "implement later"
- 每个代码步骤包含完整代码
- 每个测试步骤包含完整测试代码

### Type Consistency

- 所有组件继续使用现有 `useReducedMotion` hook
- `StepCard` / `SectionTitle` / `FeatureFrame` 接口不变
- 新组件 `ValueProp`、`Genesis`、`BackstageFrontstage` 无 props

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-10-landing-redesign-v2.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
