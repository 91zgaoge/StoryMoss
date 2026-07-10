# StoryForge 落地页重新设计实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 StoryForge 现有 `landing/` 落地页从暖赭文学风重做为「极简东方书卷」风格，新增真实产品截图与快速上手指南，并确保构建、类型检查与测试全绿。

**Architecture:** 在现有 React + Vite + Tailwind + Framer Motion 脚手架基础上，全面替换视觉 token、共享组件与页面区块；通过复用抽象组件（按钮、标题、卡片、画框）减少重复；真实截图从 `docs/product-screenshots/` 复制到 `landing/public/screenshots/` 并懒加载。

**Tech Stack:** React 18, Vite 6, TypeScript 5.8, Tailwind CSS 3, Framer Motion, lucide-react, Vitest + Testing Library

## Global Constraints

- 项目根目录：`/Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page`
- 落地页目录：`landing/`
- 所有命令默认在 `landing/` 下执行，除非另有说明
- 颜色 token 必须与设计文档一致：`parchment #f8f6f1`, `ink #1a1816`, `charcoal #6b6560`, `cinnabar #a83f2e`, `ink-line #e3ded4`
- 标题字体：`LXGW WenKai`（仅 Regular 400）
- 正文字体：系统无衬线栈
- 圆角统一 `2px`
- 必须响应 `prefers-reduced-motion`
- 每次任务完成后必须提交
- 最终必须通过 `npx vitest run`、`npm run build`、`npx tsc --noEmit`

---

## File Structure

```
landing/
  index.html                 # 可能无需改动
  package.json               # 移除 lxgw-wenkai-webfont，新增轻量字体引入方案
  tailwind.config.js         # 新颜色、新字体、新阴影
  postcss.config.js          # 不变
  vite.config.ts             # 不变
  tsconfig.json              # 不变
  src/
    index.css                # 全局字体、宣纸纹理、基础样式
    main.tsx                 # 不变
    App.tsx                  # 组装 8 个区块
    components/
      Navbar.tsx             # 固定导航 + 移动端抽屉
      Hero.tsx               # 首屏大字 + CTA
      PainPoints.tsx         # 三个痛点卡片
      Solution.tsx           # 幕后 vs 幕前双栏
      Features.tsx           # 四项功能 + 真实截图
      TimeSliced.tsx         # 分时三步骤
      QuickStart.tsx         # 新增三步上手
      DownloadCTA.tsx        # 下载号召
      Footer.tsx             # 页脚
      InkButton.tsx          # 共享按钮（替代 InkRippleButton）
      SectionTitle.tsx       # 共享区块标题
      FeatureFrame.tsx       # 装裱截图容器
      StepCard.tsx           # 步骤卡片
    hooks/
      useReducedMotion.ts    # 保持不变
    test/
      setup.ts               # 保持不变
      components/
        InkButton.test.tsx   # 替代 InkRippleButton.test.tsx
        Navbar.test.tsx      # 更新断言
        QuickStart.test.tsx  # 新增
```

---

### Task 1: 更新依赖与 Tailwind 配置

**Files:**
- Modify: `landing/package.json`
- Modify: `landing/tailwind.config.js`
- Delete: `landing/src/components/InkRippleButton.tsx`
- Delete: `landing/src/components/__tests__/InkRippleButton.test.tsx`

**Interfaces:**
- Produces: Tailwind 新 token（`parchment`, `ink`, `charcoal`, `cinnabar`, `ink-line`, `ink-wash`）和新 fontFamily（`display`, `body`）

- [ ] **Step 1: 移除重型字体依赖**

编辑 `landing/package.json`，移除 `lxgw-wenkai-webfont`：

```json
{
  "name": "storyforge-landing",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "test": "vitest run",
    "test:watch": "vitest",
    "format": "prettier --write \"src/**/*.{ts,tsx,css,json}\"",
    "format:check": "prettier --check \"src/**/*.{ts,tsx,css,json}\""
  },
  "dependencies": {
    "framer-motion": "^12.38.0",
    "lucide-react": "^0.487.0",
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@testing-library/jest-dom": "^6.9.1",
    "@testing-library/react": "^16.3.2",
    "@testing-library/user-event": "^14.6.1",
    "@types/react": "^18.3.20",
    "@types/react-dom": "^18.3.6",
    "@vitejs/plugin-react": "^4.3.4",
    "autoprefixer": "^10.4.21",
    "jsdom": "^29.0.2",
    "postcss": "^8.5.3",
    "prettier": "^3.8.3",
    "tailwindcss": "^3.4.17",
    "typescript": "^5.8.3",
    "vite": "^6.2.5",
    "vitest": "^4.1.4"
  }
}
```

- [ ] **Step 2: 安装依赖**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
rm -rf node_modules package-lock.json
npm install
```

Expected: `npm install` completes without errors.

- [ ] **Step 3: 重写 Tailwind 配置**

编辑 `landing/tailwind.config.js`：

```js
/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        parchment: '#f8f6f1',
        cream: '#fbf9f4',
        ink: '#1a1816',
        charcoal: '#6b6560',
        stone: '#827c75',
        cinnabar: '#a83f2e',
        'cinnabar-dark': '#7d2e21',
        'ink-line': '#e3ded4',
        'ink-wash': 'rgba(26, 24, 22, 0.03)',
      },
      fontFamily: {
        display: ['"LXGW WenKai"', '"Source Han Serif CN"', '"Noto Serif SC"', 'serif'],
        body: ['system-ui', '-apple-system', '"PingFang SC"', '"Microsoft YaHei"', 'sans-serif'],
      },
      boxShadow: {
        cta: '0 8px 24px rgba(168, 63, 46, 0.18)',
        card: '0 1px 3px rgba(0, 0, 0, 0.04)',
        nav: '0 1px 2px rgba(0, 0, 0, 0.04)',
      },
    },
  },
  plugins: [],
};
```

- [ ] **Step 4: 删除旧按钮组件**

Run:
```bash
rm /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/src/components/InkRippleButton.tsx
rm /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/src/components/__tests__/InkRippleButton.test.tsx
```

- [ ] **Step 5: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/package.json landing/package-lock.json landing/tailwind.config.js landing/src/components/InkRippleButton.tsx landing/src/components/__tests__/InkRippleButton.test.tsx
git commit -m "chore(landing): update deps and tailwind tokens for redesign"
```

---

### Task 2: 全局样式与字体

**Files:**
- Modify: `landing/src/index.css`
- Modify: `landing/index.html`

**Interfaces:**
- Produces: 全局宣纸纹理、字体加载策略、`font-display: swap`

- [ ] **Step 1: 更新全局 CSS**

编辑 `landing/src/index.css`：

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

@layer base {
  html {
    scroll-behavior: smooth;
  }

  body {
    @apply bg-parchment text-ink font-body antialiased;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }

  h1,
  h2,
  h3 {
    @apply font-display;
  }

  ::selection {
    background-color: rgba(168, 63, 46, 0.15);
  }
}

@layer utilities {
  .text-balance {
    text-wrap: balance;
  }

  .paper-texture {
    background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 400 400' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='noiseFilter'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23noiseFilter)'/%3E%3C/svg%3E");
    background-repeat: repeat;
    opacity: 0.04;
    pointer-events: none;
  }
}
```

- [ ] **Step 2: 在 index.html 中加载 LXGW WenKai**

编辑 `landing/index.html`，在 `<head>` 中加入：

```html
<link rel="preconnect" href="https://cdn.jsdelivr.net" crossorigin />
<link
  rel="stylesheet"
  href="https://cdn.jsdelivr.net/npm/@chinese-fonts/lxgwwenkai/dist/LXGWWenKai-Regular/result.css"
/>
```

完整 `index.html` 应类似：

```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/vite.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>StoryForge · 草苔 — AI 辅助小说创作</title>
    <link rel="preconnect" href="https://cdn.jsdelivr.net" crossorigin />
    <link
      rel="stylesheet"
      href="https://cdn.jsdelivr.net/npm/@chinese-fonts/lxgwwenkai/dist/LXGWWenKai-Regular/result.css"
    />
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 3: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/index.css landing/index.html
git commit -m "style(landing): paper texture and LXGW WenKai font loading"
```

---

### Task 3: 共享组件 — InkButton

**Files:**
- Create: `landing/src/components/InkButton.tsx`
- Create: `landing/src/components/__tests__/InkButton.test.tsx`

**Interfaces:**
- Produces: `<InkButton variant="primary" | "secondary" className? children>`

- [ ] **Step 1: 写测试**

创建 `landing/src/components/__tests__/InkButton.test.tsx`：

```tsx
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { InkButton } from '../InkButton';

describe('InkButton', () => {
  it('renders primary button', () => {
    render(<InkButton variant="primary">下载</InkButton>);
    const button = screen.getByRole('button', { name: /下载/i });
    expect(button).toBeInTheDocument();
  });

  it('renders secondary button', () => {
    render(<InkButton variant="secondary">查看</InkButton>);
    const button = screen.getByRole('button', { name: /查看/i });
    expect(button).toBeInTheDocument();
  });

  it('forwards className', () => {
    render(
      <InkButton variant="primary" className="extra-class">
        下载
      </InkButton>
    );
    const button = screen.getByRole('button', { name: /下载/i });
    expect(button.className).toContain('extra-class');
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/InkButton.test.tsx
```

Expected: FAIL — `InkButton` not found.

- [ ] **Step 3: 实现组件**

创建 `landing/src/components/InkButton.tsx`：

```tsx
import type { ReactNode, ButtonHTMLAttributes } from 'react';

interface InkButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant: 'primary' | 'secondary';
  children: ReactNode;
}

export function InkButton({ variant, className = '', children, ...rest }: InkButtonProps) {
  const base =
    'inline-flex items-center justify-center rounded-[2px] px-6 py-3 text-sm font-medium transition-colors duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cinnabar focus-visible:ring-offset-2 focus-visible:ring-offset-parchment';
  const styles =
    variant === 'primary'
      ? 'bg-cinnabar text-white hover:bg-cinnabar-dark'
      : 'border border-ink-line bg-parchment text-ink hover:border-cinnabar hover:text-cinnabar';

  return (
    <button className={`${base} ${styles} ${className}`} {...rest}>
      {children}
    </button>
  );
}
```

- [ ] **Step 4: 运行测试确认通过**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/InkButton.test.tsx
```

Expected: PASS (3 tests).

- [ ] **Step 5: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/InkButton.tsx landing/src/components/__tests__/InkButton.test.tsx
git commit -m "feat(landing): add InkButton shared component"
```

---

### Task 4: 共享组件 — SectionTitle, FeatureFrame, StepCard

**Files:**
- Create: `landing/src/components/SectionTitle.tsx`
- Create: `landing/src/components/FeatureFrame.tsx`
- Create: `landing/src/components/StepCard.tsx`

**Interfaces:**
- Produces: `<SectionTitle label title description align="center" | "left">`, `<FeatureFrame src alt>`, `<StepCard number title description>`

- [ ] **Step 1: 实现 SectionTitle**

创建 `landing/src/components/SectionTitle.tsx`：

```tsx
interface SectionTitleProps {
  label?: string;
  title: string;
  description?: string;
  align?: 'center' | 'left';
}

export function SectionTitle({ label, title, description, align = 'center' }: SectionTitleProps) {
  const alignClass = align === 'center' ? 'text-center' : 'text-left';

  return (
    <div className={`${alignClass} mb-16 md:mb-20`}>
      {label && (
        <span className="mb-3 inline-block font-mono text-xs uppercase tracking-widest text-charcoal">
          {label}
        </span>
      )}
      <h2 className="mb-4 text-[28px] leading-tight tracking-[-0.015em] text-ink md:text-[40px]">
        {title}
      </h2>
      {description && (
        <p className="mx-auto max-w-[560px] text-base leading-relaxed text-charcoal md:text-lg">
          {description}
        </p>
      )}
    </div>
  );
}
```

- [ ] **Step 2: 实现 FeatureFrame**

创建 `landing/src/components/FeatureFrame.tsx`：

```tsx
interface FeatureFrameProps {
  src: string;
  alt: string;
}

export function FeatureFrame({ src, alt }: FeatureFrameProps) {
  return (
    <div className="overflow-hidden rounded-[2px] border border-ink-line bg-cream p-2 shadow-card transition-colors duration-200 hover:border-cinnabar">
      <img
        src={src}
        alt={alt}
        loading="lazy"
        className="w-full rounded-[2px] bg-ink-wash"
      />
    </div>
  );
}
```

- [ ] **Step 3: 实现 StepCard**

创建 `landing/src/components/StepCard.tsx`：

```tsx
interface StepCardProps {
  number: string;
  title: string;
  description: string;
}

export function StepCard({ number, title, description }: StepCardProps) {
  return (
    <div className="relative">
      <span className="mb-3 block font-display text-4xl text-cinnabar/30">{number}</span>
      <h3 className="mb-2 text-lg font-medium text-ink">{title}</h3>
      <p className="leading-relaxed text-charcoal">{description}</p>
    </div>
  );
}
```

- [ ] **Step 4: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/SectionTitle.tsx landing/src/components/FeatureFrame.tsx landing/src/components/StepCard.tsx
git commit -m "feat(landing): add SectionTitle, FeatureFrame and StepCard"
```

---

### Task 5: Navbar

**Files:**
- Modify: `landing/src/components/Navbar.tsx`
- Modify: `landing/src/components/__tests__/Navbar.test.tsx`

**Interfaces:**
- Consumes: `InkButton`

- [ ] **Step 1: 更新测试**

编辑 `landing/src/components/__tests__/Navbar.test.tsx`：

```tsx
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect } from 'vitest';
import { Navbar } from '../Navbar';

describe('Navbar', () => {
  it('renders brand and links on desktop', () => {
    render(<Navbar />);
    expect(screen.getByText('草苔')).toBeInTheDocument();
    expect(screen.getByText('StoryForge')).toBeInTheDocument();
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

- [ ] **Step 2: 重写 Navbar**

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
        <a href="/" className="flex items-baseline gap-2 font-display text-xl text-ink">
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

- [ ] **Step 3: 运行测试**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/Navbar.test.tsx
```

Expected: PASS (2 tests).

- [ ] **Step 4: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/Navbar.tsx landing/src/components/__tests__/Navbar.test.tsx
git commit -m "feat(landing): redesign Navbar"
```

---

### Task 6: Hero

**Files:**
- Modify: `landing/src/components/Hero.tsx`

**Interfaces:**
- Consumes: `InkButton`, `useReducedMotion`

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
        <motion.p
          variants={reduced ? undefined : child}
          className="mb-6 font-mono text-xs uppercase tracking-widest text-charcoal"
        >
          StoryForge · 草苔
        </motion.p>

        <h1 className="mb-8 text-[40px] leading-[1.12] tracking-[-0.02em] text-ink md:text-[64px]">
          <motion.span
            variants={reduced ? undefined : child}
            className="block"
          >
            写长篇，
          </motion.span>
          <motion.span
            variants={reduced ? undefined : child}
            className="block"
          >
            先让灵感有处安放
          </motion.span>
        </h1>

        <motion.p
          variants={reduced ? undefined : child}
          className="mx-auto mb-10 max-w-[560px] text-base leading-relaxed text-charcoal md:text-lg"
        >
          草苔是 AI 随行的小说创作工作台。幕后管好角色、场景、世界观；幕前只留你和文字。
        </motion.p>

        <motion.div
          variants={reduced ? undefined : child}
          className="flex flex-col items-center justify-center gap-4 sm:flex-row"
        >
          <InkButton variant="primary">免费下载桌面版</InkButton>
          <a href="#features">
            <InkButton variant="secondary" className="group">
              看它是如何工作的
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
git commit -m "feat(landing): redesign Hero"
```

---

### Task 7: PainPoints

**Files:**
- Modify: `landing/src/components/PainPoints.tsx`

**Interfaces:**
- Consumes: `useReducedMotion`

- [ ] **Step 1: 重写 PainPoints**

编辑 `landing/src/components/PainPoints.tsx`：

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { SectionTitle } from './SectionTitle';

const pains = [
  { title: '角色写着写着崩了', description: '人设越写越散，前后言行不一致。' },
  { title: '伏笔埋了忘了回收', description: '前期线索后期无踪，读者白期待。' },
  { title: '世界观越写越矛盾', description: '设定越来越多，互相冲突难以自洽。' },
];

export function PainPoints() {
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
    <section className="mx-auto max-w-[980px] px-6 py-[100px] md:py-[160px]">
      <SectionTitle
        label="01"
        title="写到中途，往往毁于细节"
        description="灵感会再来，但角色关系、伏笔线索、世界观设定一旦失控，修改成本就会指数级上升。"
      />

      <motion.div
        initial={reduced ? undefined : 'hidden'}
        whileInView={reduced ? undefined : 'visible'}
        viewport={{ once: true, margin: '-100px' }}
        variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
        className="grid gap-6 md:grid-cols-3"
      >
        {pains.map((pain) => (
          <motion.div
            key={pain.title}
            variants={reduced ? undefined : item}
            className="group border-b border-ink-line pb-6 transition-colors duration-200 hover:border-cinnabar"
          >
            <h3 className="mb-3 text-xl text-ink">{pain.title}</h3>
            <p className="text-charcoal">{pain.description}</p>
          </motion.div>
        ))}
      </motion.div>
    </section>
  );
}
```

- [ ] **Step 2: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/PainPoints.tsx
git commit -m "feat(landing): redesign PainPoints"
```

---

### Task 8: Solution

**Files:**
- Modify: `landing/src/components/Solution.tsx`

**Interfaces:**
- Consumes: `SectionTitle`, `useReducedMotion`

- [ ] **Step 1: 重写 Solution**

编辑 `landing/src/components/Solution.tsx`：

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { SectionTitle } from './SectionTitle';

export function Solution() {
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
          label="02"
          title="两个空间，各尽其职"
          description="把创作拆成两个空间：幕后把要素结构化管好，幕前让你专注写字。"
        />

        <div className="grid gap-8 md:grid-cols-2 md:divide-x md:divide-ink-line">
          <motion.div
            initial={reduced ? undefined : 'hidden'}
            whileInView={reduced ? undefined : 'visible'}
            viewport={{ once: true, margin: '-100px' }}
            variants={reduced ? undefined : item}
            className="md:pr-10"
          >
            <h3 className="mb-3 text-2xl text-ink">幕后工作室</h3>
            <p className="leading-relaxed text-charcoal">
              管理故事、角色、场景、世界观、知识图谱。AI 帮你生成人设、追踪伏笔、分析叙事结构，让设定不崩。
            </p>
          </motion.div>

          <motion.div
            initial={reduced ? undefined : 'hidden'}
            whileInView={reduced ? undefined : 'visible'}
            viewport={{ once: true, margin: '-100px' }}
            variants={reduced ? undefined : item}
            className="md:pl-10"
          >
            <h3 className="mb-3 text-2xl text-ink">幕前写作台</h3>
            <p className="leading-relaxed text-charcoal">
              极简、全屏、自动保存。底部输入栏随时调用 AI 续写、润色、改紧张感，不打断心流。
            </p>
          </motion.div>
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
git add landing/src/components/Solution.tsx
git commit -m "feat(landing): redesign Solution"
```

---

### Task 9: Features（真实截图）

**Files:**
- Modify: `landing/src/components/Features.tsx`
- Create: `landing/public/screenshots/` (copy from `docs/product-screenshots/`)

**Interfaces:**
- Consumes: `SectionTitle`, `FeatureFrame`, `useReducedMotion`

- [ ] **Step 1: 复制产品截图**

Run:
```bash
mkdir -p /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/public/screenshots
cp /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/docs/product-screenshots/02_stories.png /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/public/screenshots/stories.png
cp /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/docs/product-screenshots/03_characters.png /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/public/screenshots/characters.png
cp /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/docs/product-screenshots/00_frontstage.png /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/public/screenshots/frontstage.png
cp /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/docs/product-screenshots/09_book-deconstruction.png /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/public/screenshots/book-deconstruction.png
```

- [ ] **Step 2: 重写 Features**

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
      '系统化人设、关系网络、知识图谱。AI 在续写时严格遵循设定，避免“角色崩坏”和“吃书”。',
    image: '/screenshots/characters.png',
    alt: '角色与世界观管理界面',
  },
  {
    title: 'AI 续写与润色',
    description:
      '底部输入栏发指令：“续写下一段”“改得更紧张”“加入意外转折”。AI 随行辅助，但创作主权始终在你。',
    image: '/screenshots/frontstage.png',
    alt: '幕前沉浸式写作界面',
  },
  {
    title: '拆书与叙事分析',
    description:
      '上传参考小说，AI 自动分析整体结构、章节节奏、角色出场频率，把“凭感觉写”变成“有参照地写”。',
    image: '/screenshots/book-deconstruction.png',
    alt: '拆书与叙事分析界面',
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
    <section id="features" className="mx-auto max-w-[980px] px-6 py-[100px] md:py-[160px]">
      <SectionTitle
        label="03"
        title="从第一行字到完本"
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
    </section>
  );
}
```

- [ ] **Step 3: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/Features.tsx landing/public/screenshots/
git commit -m "feat(landing): redesign Features with real screenshots"
```

---

### Task 10: TimeSliced

**Files:**
- Modify: `landing/src/components/TimeSliced.tsx`

**Interfaces:**
- Consumes: `SectionTitle`, `StepCard`, `useReducedMotion`

- [ ] **Step 1: 重写 TimeSliced**

编辑 `landing/src/components/TimeSliced.tsx`：

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { SectionTitle } from './SectionTitle';
import { StepCard } from './StepCard';

const steps = [
  {
    number: '01',
    title: '写作时刻',
    description: '秒出正文。只带最小必要约束，让灵感不被流程卡住。',
  },
  {
    number: '02',
    title: '审计时刻',
    description: '后台自动审校，问题以标注形式回流编辑器，当场处理小债。',
  },
  {
    number: '03',
    title: '洞察时刻',
    description: '定期产出叙事健康度报告，发现节奏与结构问题。',
  },
];

export function TimeSliced() {
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
    <section className="border-y border-ink-line bg-cream py-[100px] md:py-[160px]">
      <div className="mx-auto max-w-[980px] px-6">
        <SectionTitle
          label="04"
          title="写得快，也审得深"
          description="分时介入架构把“写”和“审”拆成三条时间线，不再让质量与速度互相拖累。"
        />

        <motion.div
          initial={reduced ? undefined : 'hidden'}
          whileInView={reduced ? undefined : 'visible'}
          viewport={{ once: true, margin: '-100px' }}
          variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
          className="relative grid gap-10 md:grid-cols-3"
        >
          <div className="absolute top-[42px] left-0 hidden h-px w-full bg-ink-line md:block" />
          {steps.map((s) => (
            <motion.div key={s.number} variants={reduced ? undefined : item}>
              <StepCard number={s.number} title={s.title} description={s.description} />
            </motion.div>
          ))}
        </motion.div>
      </div>
    </section>
  );
}
```

- [ ] **Step 2: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/TimeSliced.tsx
git commit -m "feat(landing): redesign TimeSliced"
```

---

### Task 11: QuickStart（新增）

**Files:**
- Create: `landing/src/components/QuickStart.tsx`
- Create: `landing/src/components/__tests__/QuickStart.test.tsx`

**Interfaces:**
- Consumes: `SectionTitle`, `StepCard`, `useReducedMotion`

- [ ] **Step 1: 写测试**

创建 `landing/src/components/__tests__/QuickStart.test.tsx`：

```tsx
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { QuickStart } from '../QuickStart';

describe('QuickStart', () => {
  it('renders three steps', () => {
    render(<QuickStart />);
    expect(screen.getByText('下载安装桌面版')).toBeInTheDocument();
    expect(screen.getByText('创建你的第一个故事')).toBeInTheDocument();
    expect(screen.getByText('进入幕前，写下第一段')).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/QuickStart.test.tsx
```

Expected: FAIL — `QuickStart` not found.

- [ ] **Step 3: 实现组件**

创建 `landing/src/components/QuickStart.tsx`：

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
    title: '创建你的第一个故事',
    description: '用 AI 创建故事框架，或手动填写标题与简介，从零开始。',
  },
  {
    number: '03',
    title: '进入幕前，写下第一段',
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
        label="05"
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

- [ ] **Step 4: 运行测试确认通过**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/QuickStart.test.tsx
```

Expected: PASS (1 test).

- [ ] **Step 5: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/QuickStart.tsx landing/src/components/__tests__/QuickStart.test.tsx
git commit -m "feat(landing): add QuickStart section"
```

---

### Task 12: DownloadCTA + Footer

**Files:**
- Modify: `landing/src/components/DownloadCTA.tsx`
- Modify: `landing/src/components/Footer.tsx`

**Interfaces:**
- Consumes: `InkButton`, `useReducedMotion`

- [ ] **Step 1: 重写 DownloadCTA**

编辑 `landing/src/components/DownloadCTA.tsx`：

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { InkButton } from './InkButton';

export function DownloadCTA() {
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
    <section id="download" className="border-t border-ink-line bg-cream py-[100px] text-center md:py-[160px]">
      <motion.div
        initial={reduced ? undefined : 'hidden'}
        whileInView={reduced ? undefined : 'visible'}
        viewport={{ once: true, margin: '-100px' }}
        variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
        className="mx-auto max-w-[980px] px-6"
      >
        <motion.h2
          variants={reduced ? undefined : item}
          className="mb-4 text-[32px] tracking-[-0.015em] text-ink md:text-[48px]"
        >
          开始你的第一本书
        </motion.h2>
        <motion.p
          variants={reduced ? undefined : item}
          className="mx-auto mb-8 max-w-[560px] text-lg text-charcoal"
        >
          Windows / macOS / Linux 桌面版免费下载。本地运行，数据归你。
        </motion.p>
        <motion.div variants={reduced ? undefined : item}>
          <InkButton variant="primary" className="px-10 py-4 text-base">
            立即下载
          </InkButton>
        </motion.div>
        <motion.p variants={reduced ? undefined : item} className="mt-4 text-sm text-charcoal">
          开源项目，源代码可在 GitHub 查看
        </motion.p>
      </motion.div>
    </section>
  );
}
```

- [ ] **Step 2: 重写 Footer**

编辑 `landing/src/components/Footer.tsx`：

```tsx
export function Footer() {
  return (
    <footer className="border-t border-ink-line bg-parchment py-8">
      <div className="mx-auto flex max-w-[980px] flex-col items-center justify-between gap-4 px-6 md:flex-row">
        <p className="text-sm text-charcoal">© 2026 StoryForge · 草苔</p>
        <div className="flex gap-6 text-sm text-charcoal">
          <a href="https://github.com/91zgaoge/StoryForge" className="hover:text-ink" target="_blank" rel="noreferrer">
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

- [ ] **Step 3: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/DownloadCTA.tsx landing/src/components/Footer.tsx
git commit -m "feat(landing): redesign DownloadCTA and Footer"
```

---

### Task 13: 组装 App.tsx

**Files:**
- Modify: `landing/src/App.tsx`

**Interfaces:**
- Consumes: 所有区块组件

- [ ] **Step 1: 重写 App.tsx**

编辑 `landing/src/App.tsx`：

```tsx
import { Navbar } from './components/Navbar';
import { Hero } from './components/Hero';
import { PainPoints } from './components/PainPoints';
import { Solution } from './components/Solution';
import { Features } from './components/Features';
import { TimeSliced } from './components/TimeSliced';
import { QuickStart } from './components/QuickStart';
import { DownloadCTA } from './components/DownloadCTA';
import { Footer } from './components/Footer';

export default function App() {
  return (
    <>
      <Navbar />
      <main>
        <Hero />
        <PainPoints />
        <Solution />
        <Features />
        <TimeSliced />
        <QuickStart />
        <DownloadCTA />
      </main>
      <Footer />
    </>
  );
}
```

- [ ] **Step 2: 提交**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/App.tsx
git commit -m "feat(landing): assemble redesigned landing page"
```

---

### Task 14: 全局验证与收尾

**Files:**
- Modify: `landing/.gitignore`（可选，确保 screenshots 被提交）
- Modify: `landing/README.md`（如存在，更新描述）

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

Expected: PASS（至少 6 个测试：InkButton 3 + Navbar 2 + QuickStart 1）。

- [ ] **Step 3: 运行构建**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npm run build
```

Expected: 构建成功，生成 `landing/dist/`。

- [ ] **Step 4: 检查 .gitignore**

确保 `landing/.gitignore` 没有忽略 `public/screenshots/`。默认 Vite `.gitignore` 不应忽略 `public/`。

- [ ] **Step 5: 提交验证结果**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/dist landing/README.md 2>/dev/null || true
git commit -m "chore(landing): verify build, types and tests"
```

---

## Self-Review

### Spec Coverage

| 设计文档要求 | 对应任务 |
|-------------|---------|
| 新视觉系统（色彩/字体/纹理/间距） | Task 1, Task 2 |
| 8 个页面区块 | Task 5–12 |
| 共享组件抽象 | Task 3, Task 4 |
| 真实产品截图 | Task 9 |
| 快速上手指南 | Task 11 |
| 动效与可访问性 | 各组件内置 `useReducedMotion`、focus-visible、语义化 HTML |
| 性能优化 | Task 1（移除重型字体依赖）、Task 9（图片懒加载） |
| 测试 | Task 3, 5, 11, 14 |
| 构建验证 | Task 14 |

### Placeholder Scan

- 无 "TBD", "TODO", "implement later"
- 无 "Add appropriate error handling" 等模糊描述
- 每个代码步骤包含完整代码
- 每个测试步骤包含完整测试代码

### Type Consistency

- `InkButtonProps.variant` 始终为 `"primary" | "secondary"`
- `SectionTitle.align` 始终为 `"center" | "left"`，默认 `"center"`
- `useReducedMotion()` 返回值始终作为 `reduced` 使用
- 所有 `motion` 组件的 `variants` 在 `reduced` 时为 `undefined`

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-10-landing-redesign.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
