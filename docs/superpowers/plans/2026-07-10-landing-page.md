# StoryForge 官网落地页实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `landing/` 目录下创建一个独立的 StoryForge 官网落地页，使用 React + Vite + Tailwind CSS，复用项目主技术栈，最终可构建为静态站点用于下载引导。

**Architecture:** 落地页作为独立 Vite 应用存在于项目根目录 `landing/`，不侵入 `src-frontend` 桌面应用代码。通过 npm workspace 或独立 `package.json` 管理依赖；组件按页面区块拆分；动画使用 Framer Motion 并尊重 `prefers-reduced-motion`。

**Tech Stack:** React 18 + TypeScript 5.8 + Vite 6 + Tailwind CSS 3.4 + Framer Motion + lucide-react + Vitest + Testing Library + jsdom

## Global Constraints

- 所有代码使用 TypeScript，类型检查严格模式。
- 样式策略：Tailwind CSS only，不使用 CSS Modules 或 CSS-in-JS。
- 动画不得与 CSS transition 同时作用于同一元素的同一属性。
- 图标统一使用 lucide-react，不混用其他图标库。
- 必须支持 `prefers-reduced-motion: reduce`。
- 移动端 375px 宽度下布局不溢出、文本可读、CTA 可点击。
- 遵循项目 `AGENTS.md`：每次提交需更新 `README.md`、`CHANGELOG.md` 等文档；版本号需与桌面应用对齐（如本次作为 patch 版本）。
- 修改现有代码前必须运行 GitNexus impact 分析；本计划主要新增文件，影响面低。

---

## File Structure

```
landing/
├── package.json
├── tsconfig.json
├── tsconfig.node.json
├── vite.config.ts
├── tailwind.config.js
├── postcss.config.js
├── index.html
├── src/
│   ├── main.tsx
│   ├── App.tsx
│   ├── index.css
│   ├── hooks/
│   │   └── useReducedMotion.ts
│   └── components/
│       ├── InkRippleButton.tsx
│       ├── Navbar.tsx
│       ├── Hero.tsx
│       ├── PainPoints.tsx
│       ├── Solution.tsx
│       ├── Features.tsx
│       ├── TimeSliced.tsx
│       ├── DownloadCTA.tsx
│       └── Footer.tsx
└── src/components/__tests__/
    ├── InkRippleButton.test.tsx
    └── Navbar.test.tsx
```

---

## Task 1: 初始化 landing 项目脚手架

**Files:**
- Create: `landing/package.json`
- Create: `landing/tsconfig.json`
- Create: `landing/tsconfig.node.json`
- Create: `landing/vite.config.ts`
- Create: `landing/postcss.config.js`

**Interfaces:**
- Produces: 独立的 Vite + React + TypeScript 项目，可通过 `cd landing && npm install && npm run dev` 启动。

- [ ] **Step 1: 创建 `landing/package.json`**

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
    "lxgw-wenkai-webfont": "^1.7.0",
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

- [ ] **Step 2: 创建 `landing/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

- [ ] **Step 3: 创建 `landing/tsconfig.node.json`**

```json
{
  "compilerOptions": {
    "composite": true,
    "skipLibCheck": true,
    "module": "ESNext",
    "moduleResolution": "bundler",
    "allowSyntheticDefaultImports": true
  },
  "include": ["vite.config.ts"]
}
```

- [ ] **Step 4: 创建 `landing/vite.config.ts`**

```ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: 'jsdom',
    css: false,
  },
});
```

- [ ] **Step 5: 创建 `landing/postcss.config.js`**

```js
export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
};
```

- [ ] **Step 6: 安装依赖并验证开发服务器启动**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/landing
npm install
```

Expected: `node_modules` 创建成功，无安装错误。

- [ ] **Step 7: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add landing/package.json landing/tsconfig.json landing/tsconfig.node.json landing/vite.config.ts landing/postcss.config.js
git commit -m "chore(landing): scaffold vite + react + ts project"
```

---

## Task 2: 配置 Tailwind 与全局样式

**Files:**
- Create: `landing/tailwind.config.js`
- Create: `landing/index.html`
- Create: `landing/src/index.css`
- Create: `landing/src/main.tsx`
- Create: `landing/src/App.tsx`

**Interfaces:**
- Consumes: 无。
- Produces: 设计系统中的颜色、字体、阴影 token；页面根入口。

- [ ] **Step 1: 创建 `landing/tailwind.config.js`**

```js
/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        parchment: '#f7f5ef',
        cream: '#fbf9f4',
        'terracotta-soft': '#f3e9e3',
        ink: '#2d2a26',
        charcoal: '#5a5550',
        stone: '#827c75',
        terracotta: '#b85c3e',
        'terracotta-dark': '#8f442c',
        gold: '#c9a35c',
        'ink-wash': 'rgba(45, 42, 38, 0.12)',
      },
      fontFamily: {
        display: ['"LXGW WenKai"', '"Source Han Serif CN"', '"Noto Serif SC"', 'serif'],
        body: ['"Source Han Serif CN"', '"Noto Serif SC"', '"Libre Baskerville"', 'serif'],
        sans: ['system-ui', '-apple-system', 'sans-serif'],
      },
      boxShadow: {
        cta: '0 8px 24px rgba(184, 92, 62, 0.2)',
        card: '0 2px 8px rgba(0, 0, 0, 0.04)',
        nav: '0 1px 3px rgba(0, 0, 0, 0.06)',
      },
      animation: {
        'ink-spread': 'inkSpread 0.6s ease-out forwards',
      },
      keyframes: {
        inkSpread: {
          '0%': { transform: 'scale(0)', opacity: '0.35' },
          '100%': { transform: 'scale(4)', opacity: '0' },
        },
      },
    },
  },
  plugins: [],
};
```

- [ ] **Step 2: 创建 `landing/index.html`**

```html
<!DOCTYPE html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>草苔 StoryForge — 越写越懂的 AI 小说创作系统</title>
    <meta name="description" content="AI 辅助小说创作桌面应用。幕后管理故事、角色、场景、世界观；幕前沉浸式写作。" />
    <link rel="preconnect" href="https://fonts.googleapis.com" />
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
    <link href="https://fonts.googleapis.com/css2?family=Libre+Baskerville:ital,wght@0,400;0,700;1,400&family=Sorts+Mill+Goudy:ital@0;1&display=swap" rel="stylesheet" />
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 3: 创建 `landing/src/index.css`**

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
}

@layer utilities {
  .text-balance {
    text-wrap: balance;
  }
}
```

- [ ] **Step 4: 创建 `landing/src/main.tsx`**

```tsx
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import 'lxgw-wenkai-webfont/style.css';
import './index.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

- [ ] **Step 5: 创建占位 `landing/src/App.tsx`**

```tsx
export default function App() {
  return (
    <main className="min-h-screen bg-parchment">
      <h1 className="p-10 text-center font-display text-4xl text-ink">草苔 StoryForge</h1>
    </main>
  );
}
```

- [ ] **Step 6: 验证开发服务器**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/landing
npm run dev
```

Expected: 终端显示本地 URL（如 `http://localhost:5173/`），浏览器打开后看到“草苔 StoryForge”标题，背景为羊皮纸色。

- [ ] **Step 7: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add landing/tailwind.config.js landing/index.html landing/src/index.css landing/src/main.tsx landing/src/App.tsx
git commit -m "feat(landing): add tailwind tokens, fonts, and root entry"
```

---

## Task 3: 实现 `useReducedMotion` Hook

**Files:**
- Create: `landing/src/hooks/useReducedMotion.ts`

**Interfaces:**
- Produces: `useReducedMotion(): boolean`，供动画组件消费。

- [ ] **Step 1: 创建 Hook**

```ts
import { useEffect, useState } from 'react';

export function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(false);

  useEffect(() => {
    const media = window.matchMedia('(prefers-reduced-motion: reduce)');
    setReduced(media.matches);
    const handler = (e: MediaQueryListEvent) => setReduced(e.matches);
    media.addEventListener('change', handler);
    return () => media.removeEventListener('change', handler);
  }, []);

  return reduced;
}
```

- [ ] **Step 2: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add landing/src/hooks/useReducedMotion.ts
git commit -m "feat(landing): add useReducedMotion hook"
```

---

## Task 4: 实现 InkRippleButton 组件及测试

**Files:**
- Create: `landing/src/components/InkRippleButton.tsx`
- Create: `landing/src/components/__tests__/InkRippleButton.test.tsx`

**Interfaces:**
- Consumes: Tailwind tokens `bg-terracotta`, `text-cream`, `shadow-cta`, `animate-ink-spread`。
- Produces: `<InkRippleButton variant="primary" | "secondary">{children}</InkRippleButton>`，点击时触发墨水涟漪。

- [ ] **Step 1: 创建 `landing/src/components/__tests__/InkRippleButton.test.tsx`**

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { InkRippleButton } from '../InkRippleButton';

describe('InkRippleButton', () => {
  it('renders children', () => {
    render(<InkRippleButton>下载</InkRippleButton>);
    expect(screen.getByRole('button', { name: '下载' })).toBeInTheDocument();
  });

  it('calls onClick when clicked', () => {
    const handleClick = vi.fn();
    render(<InkRippleButton onClick={handleClick}>下载</InkRippleButton>);
    fireEvent.click(screen.getByRole('button', { name: '下载' }));
    expect(handleClick).toHaveBeenCalledTimes(1);
  });

  it('renders primary variant by default', () => {
    render(<InkRippleButton>下载</InkRippleButton>);
    const button = screen.getByRole('button', { name: '下载' });
    expect(button.className).toContain('bg-terracotta');
  });

  it('renders secondary variant', () => {
    render(<InkRippleButton variant="secondary">了解更多</InkRippleButton>);
    const button = screen.getByRole('button', { name: '了解更多' });
    expect(button.className).toContain('border-terracotta');
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/landing
npx vitest run src/components/__tests__/InkRippleButton.test.tsx
```

Expected: 测试失败，提示 `InkRippleButton` 未定义或模块不存在。

- [ ] **Step 3: 创建 `landing/src/components/InkRippleButton.tsx`**

```tsx
import React, { useRef, useState } from 'react';

interface InkRippleButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  children: React.ReactNode;
  variant?: 'primary' | 'secondary';
}

export function InkRippleButton({
  children,
  variant = 'primary',
  className = '',
  onClick,
  ...rest
}: InkRippleButtonProps) {
  const [ripples, setRipples] = useState<Array<{ id: number; x: number; y: number }>>([]);
  const buttonRef = useRef<HTMLButtonElement>(null);

  const handleClick = (e: React.MouseEvent<HTMLButtonElement>) => {
    const rect = buttonRef.current?.getBoundingClientRect();
    if (rect) {
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const id = Date.now();
      setRipples((prev) => [...prev, { id, x, y }]);
      setTimeout(() => {
        setRipples((prev) => prev.filter((r) => r.id !== id));
      }, 600);
    }
    onClick?.(e);
  };

  const base =
    'relative overflow-hidden rounded-lg font-sans font-medium transition-colors duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-terracotta focus-visible:ring-offset-2 active:scale-[0.96]';
  const variants = {
    primary:
      'bg-terracotta text-cream px-8 py-3.5 shadow-cta hover:bg-terracotta-dark',
    secondary:
      'border-[1.5px] border-terracotta text-terracotta-dark bg-transparent px-8 py-3.5 hover:bg-terracotta-soft',
  };

  return (
    <button
      ref={buttonRef}
      className={`${base} ${variants[variant]} ${className}`}
      onClick={handleClick}
      {...rest}
    >
      {children}
      {ripples.map((r) => (
        <span
          key={r.id}
          className="pointer-events-none absolute animate-ink-spread rounded-full bg-ink-wash"
          style={{
            left: r.x,
            top: r.y,
            width: 20,
            height: 20,
            marginLeft: -10,
            marginTop: -10,
          }}
        />
      ))}
    </button>
  );
}
```

- [ ] **Step 4: 运行测试确认通过**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/landing
npx vitest run src/components/__tests__/InkRippleButton.test.tsx
```

Expected: 4 个测试全部通过。

- [ ] **Step 5: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add landing/src/components/InkRippleButton.tsx landing/src/components/__tests__/InkRippleButton.test.tsx
git commit -m "feat(landing): add InkRippleButton with tests"
```

---

## Task 5: 实现 Navbar 组件及测试

**Files:**
- Create: `landing/src/components/Navbar.tsx`
- Create: `landing/src/components/__tests__/Navbar.test.tsx`

**Interfaces:**
- Consumes: `InkRippleButton`。
- Produces: 固定顶部导航栏，支持桌面链接和移动端汉堡菜单。

- [ ] **Step 1: 创建 `landing/src/components/__tests__/Navbar.test.tsx`**

```tsx
import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Navbar } from '../Navbar';

describe('Navbar', () => {
  it('renders brand name', () => {
    render(<Navbar />);
    expect(screen.getByText('草苔')).toBeInTheDocument();
    expect(screen.getByText('StoryForge')).toBeInTheDocument();
  });

  it('renders desktop download button', () => {
    render(<Navbar />);
    expect(screen.getByRole('button', { name: '免费下载' })).toBeInTheDocument();
  });

  it('toggles mobile menu', () => {
    render(<Navbar />);
    const toggle = screen.getByLabelText('打开菜单');
    fireEvent.click(toggle);
    expect(screen.getByLabelText('关闭菜单')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '免费下载' })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/landing
npx vitest run src/components/__tests__/Navbar.test.tsx
```

Expected: 失败，提示 `Navbar` 未找到。

- [ ] **Step 3: 创建 `landing/src/components/Navbar.tsx`**

```tsx
import { useState, useEffect } from 'react';
import { Menu, X } from 'lucide-react';
import { InkRippleButton } from './InkRippleButton';

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
    { href: '#download', label: '下载' },
  ];

  return (
    <header
      className={`fixed top-0 left-0 right-0 z-50 h-[72px] transition-shadow duration-200 ${
        scrolled ? 'shadow-nav' : ''
      } border-b border-[#ddd7cd] bg-parchment/95 backdrop-blur-sm`}
    >
      <nav
        className="mx-auto flex h-full max-w-[1100px] items-center justify-between px-6"
        aria-label="主导航"
      >
        <a href="/" className="flex items-center gap-2 font-display text-xl text-ink">
          <span className="text-terracotta">草苔</span>
          <span className="font-sans text-sm tracking-wide text-stone">StoryForge</span>
        </a>

        <div className="hidden items-center gap-8 md:flex">
          {links.map((l) => (
            <a
              key={l.href}
              href={l.href}
              className="font-sans text-sm text-charcoal transition-colors hover:text-ink"
            >
              {l.label}
            </a>
          ))}
          <InkRippleButton variant="primary" className="px-5 py-2 text-sm">
            免费下载
          </InkRippleButton>
        </div>

        <button
          className="flex h-11 w-11 items-center justify-center rounded-md text-ink md:hidden"
          aria-label={mobileOpen ? '关闭菜单' : '打开菜单'}
          aria-expanded={mobileOpen}
          onClick={() => setMobileOpen((s) => !s)}
        >
          {mobileOpen ? <X size={24} /> : <Menu size={24} />}
        </button>
      </nav>

      {mobileOpen && (
        <div className="absolute left-0 right-0 top-[72px] border-b border-[#ddd7cd] bg-parchment px-6 py-4 md:hidden">
          <ul className="flex flex-col gap-4">
            {links.map((l) => (
              <li key={l.href}>
                <a
                  href={l.href}
                  className="block font-sans text-charcoal hover:text-ink"
                  onClick={() => setMobileOpen(false)}
                >
                  {l.label}
                </a>
              </li>
            ))}
            <li>
              <InkRippleButton variant="primary" className="w-full">
                免费下载
              </InkRippleButton>
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
cd /Users/yuzaimu/projects/StoryForge/landing
npx vitest run src/components/__tests__/Navbar.test.tsx
```

Expected: 3 个测试全部通过。

- [ ] **Step 5: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add landing/src/components/Navbar.tsx landing/src/components/__tests__/Navbar.test.tsx
git commit -m "feat(landing): add Navbar with mobile menu and tests"
```

---

## Task 6: 实现 Hero 区域

**Files:**
- Create: `landing/src/components/Hero.tsx`

**Interfaces:**
- Consumes: `InkRippleButton`, `useReducedMotion`。
- Produces: 全屏 Hero 区域，标题按词组动画显现。

- [ ] **Step 1: 创建 `landing/src/components/Hero.tsx`**

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { InkRippleButton } from './InkRippleButton';
import { ChevronDown } from 'lucide-react';

export function Hero() {
  const reduced = useReducedMotion();

  const titleWords = ['把混沌的长篇，', '写成有序的小说'];

  const container = {
    hidden: {},
    visible: {
      transition: { staggerChildren: 0.12 },
    },
  };

  const child = {
    hidden: { opacity: 0, y: 12, filter: 'blur(4px)' },
    visible: {
      opacity: 1,
      y: 0,
      filter: 'blur(0px)',
      transition: { duration: 0.6, ease: [0.16, 1, 0.3, 1] },
    },
  };

  return (
    <section className="relative flex min-h-screen flex-col items-center justify-center px-6 pt-[72px] text-center">
      <div className="absolute inset-0 -z-10 overflow-hidden">
        <div className="absolute left-1/2 top-1/2 h-[600px] w-[600px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-terracotta-soft/40 blur-3xl" />
      </div>

      <motion.div
        variants={reduced ? undefined : container}
        initial="hidden"
        animate="visible"
        className="max-w-[720px]"
      >
        <motion.p
          variants={reduced ? undefined : child}
          className="mb-4 font-sans text-sm tracking-widest text-stone"
        >
          StoryForge · 草苔
        </motion.p>

        <h1 className="mb-6 text-[40px] leading-[1.15] tracking-[-0.02em] text-ink md:text-[56px]">
          {titleWords.map((word, i) => (
            <motion.span
              key={i}
              variants={reduced ? undefined : child}
              className="inline-block"
              style={{ marginRight: '0.25em' }}
            >
              {word}
            </motion.span>
          ))}
        </h1>

        <motion.p
          variants={reduced ? undefined : child}
          className="mx-auto mb-10 max-w-[640px] text-lg leading-relaxed text-charcoal md:text-xl"
        >
          AI 在需要时随行辅助。幕后管理故事、角色、场景、世界观；幕前沉浸式写作。
        </motion.p>

        <motion.div
          variants={reduced ? undefined : child}
          className="flex flex-col items-center justify-center gap-4 sm:flex-row"
        >
          <InkRippleButton variant="primary">免费下载桌面版</InkRippleButton>
          <a href="#features">
            <InkRippleButton variant="secondary" className="group">
              查看功能
              <ChevronDown
                className="ml-1 inline-block transition-transform group-hover:translate-y-0.5"
                size={16}
              />
            </InkRippleButton>
          </a>
        </motion.div>
      </motion.div>
    </section>
  );
}
```

- [ ] **Step 2: 在浏览器验证 Hero**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/landing
npm run dev
```

Expected: 首屏显示“把混沌的长篇，写成有序的小说”，副标题和两个按钮可见，标题有淡入动画。

- [ ] **Step 3: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add landing/src/components/Hero.tsx
git commit -m "feat(landing): add Hero section with staggered ink reveal"
```

---

## Task 7: 实现 PainPoints 区域

**Files:**
- Create: `landing/src/components/PainPoints.tsx`

**Interfaces:**
- Consumes: `useReducedMotion`。
- Produces: 痛点区域，使用引言 + 边注布局。

- [ ] **Step 1: 创建 `landing/src/components/PainPoints.tsx`**

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';

export function PainPoints() {
  const reduced = useReducedMotion();

  const variants = {
    hidden: { opacity: 0, y: 16 },
    visible: {
      opacity: 1,
      y: 0,
      transition: { duration: 0.6, ease: [0.16, 1, 0.3, 1] },
    },
  };

  const pains = [
    '角色写着写着就崩了',
    '伏笔埋了却忘了回收',
    '设定越写越自相矛盾',
  ];

  return (
    <section className="mx-auto max-w-[1100px] px-6 py-[120px] md:py-[160px]">
      <div className="grid gap-12 md:grid-cols-12 md:gap-8">
        <motion.div
          initial={reduced ? undefined : 'hidden'}
          whileInView={reduced ? undefined : 'visible'}
          viewport={{ once: true, margin: '-100px' }}
          variants={reduced ? undefined : variants}
          className="md:col-span-7"
        >
          <h2 className="mb-6 text-[32px] leading-tight tracking-[-0.015em] text-ink md:text-[40px]">
            写长篇，最怕的不是没灵感
          </h2>
          <p className="max-w-[560px] text-lg leading-relaxed text-charcoal">
            灵感会再来，但角色关系、伏笔线索、世界观设定一旦失控，修改成本就会指数级上升。
            大多数作者不是缺想法，而是缺一个让想法不散架的系统。
          </p>
        </motion.div>

        <motion.aside
          initial={reduced ? undefined : 'hidden'}
          whileInView={reduced ? undefined : 'visible'}
          viewport={{ once: true, margin: '-100px' }}
          variants={reduced ? undefined : { ...variants, visible: { ...variants.visible, transition: { ...variants.visible.transition, delay: 0.15 } } }}
          className="md:col-span-5"
          aria-label="常见创作痛点"
        >
          <div className="mb-4 h-[2px] w-10 bg-gold" />
          <ul className="space-y-4 font-display text-xl text-ink">
            {pains.map((pain) => (
              <li key={pain} className="flex items-baseline gap-3">
                <span className="text-gold">·</span>
                {pain}
              </li>
            ))}
          </ul>
        </motion.aside>
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add landing/src/components/PainPoints.tsx
git commit -m "feat(landing): add PainPoints section"
```

---

## Task 8: 实现 Solution 区域

**Files:**
- Create: `landing/src/components/Solution.tsx`

**Interfaces:**
- Consumes: `useReducedMotion`。
- Produces: 展示“幕后规划，幕前写作”双空间概念的区域。

- [ ] **Step 1: 创建 `landing/src/components/Solution.tsx`**

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';

export function Solution() {
  const reduced = useReducedMotion();

  const item = {
    hidden: { opacity: 0, y: 20 },
    visible: {
      opacity: 1,
      y: 0,
      transition: { duration: 0.6, ease: [0.16, 1, 0.3, 1] },
    },
  };

  return (
    <section id="approach" className="bg-cream py-[120px] md:py-[160px]">
      <div className="mx-auto max-w-[1100px] px-6">
        <motion.div
          initial={reduced ? undefined : 'hidden'}
          whileInView={reduced ? undefined : 'visible'}
          viewport={{ once: true, margin: '-100px' }}
          variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.12 } } }}
          className="mb-16 text-center"
        >
          <motion.h2
            variants={reduced ? undefined : item}
            className="mb-4 text-[32px] tracking-[-0.015em] text-ink md:text-[40px]"
          >
            幕后规划，幕前写作
          </motion.h2>
          <motion.p variants={reduced ? undefined : item} className="mx-auto max-w-[640px] text-lg text-charcoal">
            把创作拆成两个空间：幕后把要素结构化管好，幕前让你专注写字。
          </motion.p>
        </motion.div>

        <div className="grid gap-8 md:grid-cols-2">
          <motion.div
            initial={reduced ? undefined : 'hidden'}
            whileInView={reduced ? undefined : 'visible'}
            viewport={{ once: true, margin: '-100px' }}
            variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
            className="rounded-xl bg-parchment p-8 shadow-card"
          >
            <motion.h3 variants={reduced ? undefined : item} className="mb-3 text-2xl text-ink">
              幕后工作室
            </motion.h3>
            <motion.p variants={reduced ? undefined : item} className="leading-relaxed text-charcoal">
              管理故事、角色、场景、世界观、知识图谱。AI 帮你生成人设、追踪伏笔、分析叙事结构，让设定不崩。
            </motion.p>
          </motion.div>

          <motion.div
            initial={reduced ? undefined : 'hidden'}
            whileInView={reduced ? undefined : 'visible'}
            viewport={{ once: true, margin: '-100px' }}
            variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1, delayChildren: 0.15 } } }}
            className="rounded-xl bg-parchment p-8 shadow-card"
          >
            <motion.h3 variants={reduced ? undefined : item} className="mb-3 text-2xl text-ink">
              幕前写作台
            </motion.h3>
            <motion.p variants={reduced ? undefined : item} className="leading-relaxed text-charcoal">
              极简、全屏、自动保存。底部输入栏随时调用 AI 续写、润色、改紧张感，不打断心流。
            </motion.p>
          </motion.div>
        </div>
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add landing/src/components/Solution.tsx
git commit -m "feat(landing): add Solution section with backstage/frontstage cards"
```

---

## Task 9: 实现 Features 长卷区域

**Files:**
- Create: `landing/src/components/Features.tsx`

**Interfaces:**
- Consumes: `useReducedMotion`。
- Produces: 四个功能段落，图文左右交替。

- [ ] **Step 1: 创建 `landing/src/components/Features.tsx`**

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { BookOpen, Users, Globe, Sparkles } from 'lucide-react';

const features = [
  {
    icon: BookOpen,
    title: '故事与场景管理',
    description:
      '把一本小说拆成可管理的故事、章节、场景。每个场景都有出场角色、地点、状态，降低“写一章”的心理压力。',
  },
  {
    icon: Users,
    title: '角色与世界观',
    description:
      '系统化人设、关系网络、知识图谱。AI 在续写时严格遵循设定，避免“角色崩坏”和“吃书”。',
  },
  {
    icon: Sparkles,
    title: 'AI 续写与润色',
    description:
      '底部输入栏发指令：“续写下一段”“改得更紧张”“加入意外转折”。AI 随行辅助，但创作主权始终在你。',
  },
  {
    icon: Globe,
    title: '拆书与分析',
    description:
      '上传参考小说，AI 自动分析整体结构、章节节奏、角色出场频率，把“凭感觉写”变成“有参照地写”。',
  },
];

export function Features() {
  const reduced = useReducedMotion();

  const item = {
    hidden: { opacity: 0, y: 20 },
    visible: {
      opacity: 1,
      y: 0,
      transition: { duration: 0.6, ease: [0.16, 1, 0.3, 1] },
    },
  };

  return (
    <section id="features" className="mx-auto max-w-[1100px] px-6 py-[120px] md:py-[160px]">
      <motion.div
        initial={reduced ? undefined : 'hidden'}
        whileInView={reduced ? undefined : 'visible'}
        viewport={{ once: true, margin: '-100px' }}
        variants={reduced ? undefined : item}
        className="mb-16 text-center"
      >
        <h2 className="mb-4 text-[32px] tracking-[-0.015em] text-ink md:text-[40px]">
          一套完整的创作系统
        </h2>
        <p className="mx-auto max-w-[640px] text-lg text-charcoal">
          从灵感、规划、写作到分析，草苔把长篇小说创作的每个环节都装进了工作台。
        </p>
      </motion.div>

      <div className="space-y-24 md:space-y-32">
        {features.map((f, idx) => {
          const Icon = f.icon;
          const isReversed = idx % 2 === 1;

          return (
            <motion.div
              key={f.title}
              initial={reduced ? undefined : 'hidden'}
              whileInView={reduced ? undefined : 'visible'}
              viewport={{ once: true, margin: '-100px' }}
              variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.12 } } }}
              className={`grid items-center gap-10 md:grid-cols-2 ${isReversed ? 'md:flex-row-reverse' : ''}`}
            >
              <motion.div
                variants={reduced ? undefined : item}
                className={isReversed ? 'md:order-2' : ''}
              >
                <div className="mb-4 inline-flex h-12 w-12 items-center justify-center rounded-lg bg-terracotta-soft text-terracotta">
                  <Icon size={24} />
                </div>
                <h3 className="mb-3 text-2xl text-ink md:text-3xl">{f.title}</h3>
                <p className="max-w-[480px] text-lg leading-relaxed text-charcoal">
                  {f.description}
                </p>
              </motion.div>

              <motion.div
                variants={reduced ? undefined : item}
                className={`rounded-xl bg-cream p-8 shadow-card ${isReversed ? 'md:order-1' : ''}`}
              >
                <div className="aspect-[4/3] rounded-lg bg-terracotta-soft/50" aria-label={`${f.title} 占位示意图`} />
              </motion.div>
            </motion.div>
          );
        })}
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add landing/src/components/Features.tsx
git commit -m "feat(landing): add alternating Features section"
```

---

## Task 10: 实现 TimeSliced 强调区域

**Files:**
- Create: `landing/src/components/TimeSliced.tsx`

**Interfaces:**
- Consumes: `useReducedMotion`。
- Produces: 分时介入架构的三步骤强调区。

- [ ] **Step 1: 创建 `landing/src/components/TimeSliced.tsx`**

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';

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
      transition: { duration: 0.6, ease: [0.16, 1, 0.3, 1] },
    },
  };

  return (
    <section className="bg-terracotta-soft py-[120px] md:py-[160px]">
      <div className="mx-auto max-w-[1100px] px-6">
        <motion.div
          initial={reduced ? undefined : 'hidden'}
          whileInView={reduced ? undefined : 'visible'}
          viewport={{ once: true, margin: '-100px' }}
          variants={reduced ? undefined : item}
          className="mb-16 text-center"
        >
          <h2 className="mb-4 text-[32px] tracking-[-0.015em] text-ink md:text-[40px]">
            写得快，也审得深
          </h2>
          <p className="mx-auto max-w-[640px] text-lg text-charcoal">
            分时介入架构把“写”和“审”拆成三条时间线，不再让质量与速度互相拖累。
          </p>
        </motion.div>

        <motion.div
          initial={reduced ? undefined : 'hidden'}
          whileInView={reduced ? undefined : 'visible'}
          viewport={{ once: true, margin: '-100px' }}
          variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.12 } } }}
          className="grid gap-8 md:grid-cols-3"
        >
          {steps.map((s) => (
            <motion.div key={s.number} variants={reduced ? undefined : item} className="text-center md:text-left">
              <span className="mb-3 block font-display text-4xl text-gold">{s.number}</span>
              <h3 className="mb-2 text-xl text-ink">{s.title}</h3>
              <p className="leading-relaxed text-charcoal">{s.description}</p>
            </motion.div>
          ))}
        </motion.div>
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add landing/src/components/TimeSliced.tsx
git commit -m "feat(landing): add TimeSliced highlight section"
```

---

## Task 11: 实现 DownloadCTA 与 Footer

**Files:**
- Create: `landing/src/components/DownloadCTA.tsx`
- Create: `landing/src/components/Footer.tsx`

**Interfaces:**
- Consumes: `InkRippleButton`。
- Produces: 最终下载转化区和页脚。

- [ ] **Step 1: 创建 `landing/src/components/DownloadCTA.tsx`**

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { InkRippleButton } from './InkRippleButton';

export function DownloadCTA() {
  const reduced = useReducedMotion();

  const item = {
    hidden: { opacity: 0, y: 20 },
    visible: {
      opacity: 1,
      y: 0,
      transition: { duration: 0.6, ease: [0.16, 1, 0.3, 1] },
    },
  };

  return (
    <section id="download" className="mx-auto max-w-[1100px] px-6 py-[120px] text-center md:py-[160px]">
      <motion.div
        initial={reduced ? undefined : 'hidden'}
        whileInView={reduced ? undefined : 'visible'}
        viewport={{ once: true, margin: '-100px' }}
        variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.12 } } }}
      >
        <motion.h2
          variants={reduced ? undefined : item}
          className="mb-4 text-[32px] tracking-[-0.015em] text-ink md:text-[48px]"
        >
          开始你的第一本书
        </motion.h2>
        <motion.p variants={reduced ? undefined : item} className="mx-auto mb-10 max-w-[560px] text-lg text-charcoal">
          Windows / macOS / Linux 桌面版免费下载。本地运行，数据归你。
        </motion.p>
        <motion.div variants={reduced ? undefined : item}>
          <InkRippleButton variant="primary" className="px-10 py-4 text-lg">
            立即下载
          </InkRippleButton>
        </motion.div>
        <motion.p variants={reduced ? undefined : item} className="mt-4 text-sm text-stone">
          开源项目，源代码可在 GitHub 查看
        </motion.p>
      </motion.div>
    </section>
  );
}
```

- [ ] **Step 2: 创建 `landing/src/components/Footer.tsx`**

```tsx
export function Footer() {
  return (
    <footer className="border-t border-[#ddd7cd] bg-parchment py-10">
      <div className="mx-auto flex max-w-[1100px] flex-col items-center justify-between gap-4 px-6 md:flex-row">
        <p className="text-sm text-stone">
          © 2026 StoryForge Team. 保留所有权利。
        </p>
        <nav className="flex gap-6" aria-label="页脚链接">
          <a href="https://github.com/91zgaoge/StoryForge" className="text-sm text-charcoal hover:text-ink">
            GitHub
          </a>
          <a href="#" className="text-sm text-charcoal hover:text-ink">
            用户指南
          </a>
          <a href="#" className="text-sm text-charcoal hover:text-ink">
            更新日志
          </a>
        </nav>
      </div>
    </footer>
  );
}
```

- [ ] **Step 3: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add landing/src/components/DownloadCTA.tsx landing/src/components/Footer.tsx
git commit -m "feat(landing): add download CTA and footer"
```

---

## Task 12: 组装 App.tsx 并验证完整页面

**Files:**
- Modify: `landing/src/App.tsx`

**Interfaces:**
- Consumes: `Navbar`, `Hero`, `PainPoints`, `Solution`, `Features`, `TimeSliced`, `DownloadCTA`, `Footer`。
- Produces: 完整落地页应用。

- [ ] **Step 1: 修改 `landing/src/App.tsx`**

```tsx
import { Navbar } from './components/Navbar';
import { Hero } from './components/Hero';
import { PainPoints } from './components/PainPoints';
import { Solution } from './components/Solution';
import { Features } from './components/Features';
import { TimeSliced } from './components/TimeSliced';
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
        <DownloadCTA />
      </main>
      <Footer />
    </>
  );
}
```

- [ ] **Step 2: 类型检查与测试**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/landing
npx tsc --noEmit
npx vitest run
```

Expected: TypeScript 无错误，所有测试通过。

- [ ] **Step 3: 浏览器验证**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/landing
npm run dev
```

Expected: 页面从上到下完整渲染，导航可点击、移动端菜单可展开、Hero 动画正常、各区块滚动时出现。

- [ ] **Step 4: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add landing/src/App.tsx
git commit -m "feat(landing): assemble full landing page"
```

---

## Task 13: 生产构建与项目级集成

**Files:**
- Modify: `landing/package.json`（新增 build 输出配置无需修改，但需验证）
- Modify: 项目根 `README.md`（新增 landing 说明段落）
- Modify: `CHANGELOG.md`（新增版本条目）

**Interfaces:**
- Produces: `landing/dist/` 静态产物；项目文档更新。

- [ ] **Step 1: 生产构建验证**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/landing
npm run build
```

Expected: `landing/dist/` 目录生成，包含 `index.html` 和静态资源，无构建错误。

- [ ] **Step 2: 预览构建产物**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge/landing
npm run preview
```

Expected: 预览服务器正常，页面可访问。

- [ ] **Step 3: 更新项目根 `README.md`（在“安装与运行”后新增 landing 段落）**

在 `README.md` 的 `## 🚀 安装与运行` 章节末尾追加：

```markdown
### 构建官网落地页

```bash
cd landing
npm install
npm run build
```

构建产物位于 `landing/dist/`，可部署到任意静态托管服务。
```

- [ ] **Step 4: 更新 `CHANGELOG.md`**

在 CHANGELOG 顶部新增条目（版本号与当前桌面应用版本对齐，例如 v0.26.59）：

```markdown
## v0.26.59

- **官网落地页**: 新增 `landing/` 独立站点，采用暖赭文学风格，面向新用户引导下载桌面版。
```

- [ ] **Step 5: 运行项目级检查**

Run:
```bash
cd /Users/yuzaimu/projects/StoryForge
cd src-frontend && npx tsc --noEmit && cd ..
npx vitest run
python3 scripts/architecture_guard.py
```

Expected: 桌面应用前端类型检查通过；vitest 通过；architecture_guard 通过。

- [ ] **Step 6: Commit**

```bash
cd /Users/yuzaimu/projects/StoryForge
git add README.md CHANGELOG.md landing/dist/.gitkeep
# 注意：dist 目录通常应加入 .gitignore，不提交构建产物
git add landing/.gitignore
git commit -m "docs: update README and CHANGELOG for landing page"
```

- [ ] **Step 7: 添加 `landing/.gitignore` 忽略构建产物**

```
node_modules
dist
*.local
```

---

## 验收标准

- [ ] `cd landing && npm install && npm run dev` 启动成功。
- [ ] `cd landing && npm run build` 生成 `landing/dist/` 且无错误。
- [ ] `cd landing && npx vitest run` 全部通过。
- [ ] `cd landing && npx tsc --noEmit` 无类型错误。
- [ ] 首屏在 3 秒内明确传递产品价值。
- [ ] 移动端 375px 宽度下无溢出、CTA 可点击。
- [ ] 动画在 `prefers-reduced-motion: reduce` 下降级。
- [ ] 项目级 `npx tsc --noEmit`、`npx vitest run`、`architecture_guard.py` 无回归。
