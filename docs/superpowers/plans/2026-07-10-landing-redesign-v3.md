# StoryForge 落地页 v3 重新设计实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在保留现有「极简东方书卷」视觉系统与已完成的 LOGO/截图资源基础上，根据 v3 设计文档全面重写落地页文案，新增 `WhyStoryForge` 技术优势区块，重组 `App.tsx` 页面结构，修复现有失败测试，通过类型检查与测试，并最终部署到 ai.91z.net。

**Architecture:** 复用现有 React + Vite + Tailwind + Framer Motion 组件体系；每个区块一个独立组件；`App.tsx` 按新顺序组装；文案直接来自项目文档；新增组件同步编写测试。

**Tech Stack:** React 18, Vite 6, TypeScript 5.8, Tailwind CSS 3, Framer Motion, lucide-react, basic-ftp

## Global Constraints

- 项目根目录：`/Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page`
- 落地页目录：`landing/`
- 所有命令默认在 `landing/` 下执行，部署命令除外。
- 颜色、字体、圆角等视觉 token 与 v2 保持一致。
- 必须响应 `prefers-reduced-motion`（通过 `useReducedMotion` hook）。
- 每次任务完成后必须提交。
- 最终必须通过 `npx vitest run`、`npm run build`、`npx tsc --noEmit`。
- 部署到 ai.91z.net 需使用 `npm run deploy`（FTP 凭据已通过环境变量传入）。

---

## File Structure

已存在且本次会修改的文件：
- `landing/src/components/Hero.tsx` — 首屏 Hero，调整副标题与信任小字。
- `landing/src/components/ValueProp.tsx` — 一句话价值主张。
- `landing/src/components/PainPoints.tsx` — 痛点区块。
- `landing/src/components/BackstageFrontstage.tsx` — 双空间对比。
- `landing/src/components/Genesis.tsx` — 创世四步流程。
- `landing/src/components/TimeSliced.tsx` — 分时介入三条时间线。
- `landing/src/components/Features.tsx` — 6 项功能展示。
- `landing/src/components/QuickStart.tsx` — 三步上手。
- `landing/src/components/__tests__/QuickStart.test.tsx` — 同步更新断言。
- `landing/src/components/Footer.tsx` — 加入 LOGO。
- `landing/src/App.tsx` — 按新结构组装，移除 `Solution`，新增 `WhyStoryForge`。

本次新增文件：
- `landing/src/components/WhyStoryForge.tsx` — 三大技术优势。
- `landing/src/components/__tests__/WhyStoryForge.test.tsx` — 对应测试。

---

### Task 1: 更新 Hero 文案

**Files:**
- Modify: `landing/src/components/Hero.tsx`

**Interfaces:**
- Consumes: `InkButton`, `useReducedMotion`
- Produces: `<Hero>`

- [ ] **Step 1: 重写 Hero 文案**

编辑 `landing/src/components/Hero.tsx`，完整内容如下：

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
          草苔 StoryForge 是专为长篇小说作者打造的系统工作台。幕后管理角色、场景、世界观；幕前沉浸式写作；AI 随行辅助，但不抢戏。
        </motion.p>

        <motion.p
          variants={reduced ? undefined : child}
          className="mx-auto mb-10 max-w-[600px] text-sm text-stone"
        >
          v0.26.58 · 本地运行 · 开源可审计
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

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/Hero.tsx
git commit -m "feat(landing): update Hero copy for v3"
```

---

### Task 2: 更新 ValueProp 文案

**Files:**
- Modify: `landing/src/components/ValueProp.tsx`

**Interfaces:**
- Consumes: `useReducedMotion`
- Produces: `<ValueProp>`

- [ ] **Step 1: 重写 ValueProp 文案**

编辑 `landing/src/components/ValueProp.tsx`：

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
        草苔不是聊天式 AI，而是一套把「灵感 → 规划 → 写作 → 审校」串起来的长篇小说创作系统。
      </motion.p>
    </section>
  );
}
```

- [ ] **Step 2: 运行测试确认通过**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/ValueProp.test.tsx
```

Expected: PASS (1 test).

- [ ] **Step 3: 提交**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/ValueProp.tsx
git commit -m "feat(landing): update ValueProp copy for v3"
```

---

### Task 3: 更新 PainPoints 文案

**Files:**
- Modify: `landing/src/components/PainPoints.tsx`

**Interfaces:**
- Consumes: `SectionTitle`, `useReducedMotion`
- Produces: `<PainPoints>`

- [ ] **Step 1: 重写 PainPoints 文案**

编辑 `landing/src/components/PainPoints.tsx`：

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { SectionTitle } from './SectionTitle';

const pains = [
  { title: '角色写着写着崩了', description: '人设越写越散，前后言行不一致，读者直呼“换作者了”。' },
  { title: '伏笔埋了忘了回收', description: '前期精彩线索后期无踪，期待落空，故事虎头蛇尾。' },
  { title: '世界观越写越矛盾', description: '设定越堆越多，互相冲突难以自洽，回头改成本巨大。' },
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

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/PainPoints.tsx
git commit -m "feat(landing): update PainPoints copy for v3"
```

---

### Task 4: 更新 BackstageFrontstage 文案

**Files:**
- Modify: `landing/src/components/BackstageFrontstage.tsx`

**Interfaces:**
- Consumes: `SectionTitle`, `FeatureFrame`, `useReducedMotion`
- Produces: `<BackstageFrontstage>`

- [ ] **Step 1: 重写 BackstageFrontstage 文案**

编辑 `landing/src/components/BackstageFrontstage.tsx`：

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
          label="02"
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
                <li>管理故事、角色、场景、世界观，把创作要素结构化管好。</li>
                <li>知识图谱可视化人物、地点、事件关系，一眼发现谁太久没出场。</li>
                <li>伏笔看板追踪线索的埋下与回收，防止烂尾。</li>
                <li>AI 模型、提示词注册表、创作方法论统一配置。</li>
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
                <li>极简、全屏、自动保存，无干扰码字环境。</li>
                <li>底部输入栏随时调用 AI 续写、润色、改紧张感。</li>
                <li>文思模式切换 AI 介入程度，被动或主动辅助。</li>
                <li>创作主权在你，AI 只在需要时随行。</li>
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

- [ ] **Step 2: 提交**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/BackstageFrontstage.tsx
git commit -m "feat(landing): update BackstageFrontstage copy for v3"
```

---

### Task 5: 更新 Genesis 文案

**Files:**
- Modify: `landing/src/components/Genesis.tsx`

**Interfaces:**
- Consumes: `SectionTitle`, `StepCard`, `useReducedMotion`
- Produces: `<Genesis>`

- [ ] **Step 1: 重写 Genesis 文案**

编辑 `landing/src/components/Genesis.tsx`：

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
    description: '自动进入幕前写作台，第一章正文已就绪，可立即修改与续写。',
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
        label="03"
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

- [ ] **Step 2: 运行测试确认通过**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/Genesis.test.tsx
```

Expected: PASS (1 test).

- [ ] **Step 3: 提交**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/Genesis.tsx
git commit -m "feat(landing): update Genesis copy for v3"
```

---

### Task 6: 更新 TimeSliced 文案

**Files:**
- Modify: `landing/src/components/TimeSliced.tsx`

**Interfaces:**
- Consumes: `SectionTitle`, `StepCard`, `useReducedMotion`
- Produces: `<TimeSliced>`

- [ ] **Step 1: 重写 TimeSliced 文案**

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
    description: '后台 7 维 Inspector 异步审校，问题以红黄蓝标注回流编辑器，当场处理小债。',
  },
  {
    number: '03',
    title: '洞察时刻',
    description: '定期产出叙事健康度报告，节奏、戏份、结构问题一目了然。',
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

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/TimeSliced.tsx
git commit -m "feat(landing): update TimeSliced copy for v3"
```

---

### Task 7: 新增 WhyStoryForge 组件

**Files:**
- Create: `landing/src/components/WhyStoryForge.tsx`
- Create: `landing/src/components/__tests__/WhyStoryForge.test.tsx`

**Interfaces:**
- Consumes: `SectionTitle`, `useReducedMotion`
- Produces: `<WhyStoryForge>`

- [ ] **Step 1: 写测试**

创建 `landing/src/components/__tests__/WhyStoryForge.test.tsx`：

```tsx
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { WhyStoryForge } from '../WhyStoryForge';

describe('WhyStoryForge', () => {
  it('renders three advantage cards', () => {
    render(<WhyStoryForge />);
    expect(screen.getByText('长上下文不丢约束')).toBeInTheDocument();
    expect(screen.getByText('稳定压倒灵感')).toBeInTheDocument();
    expect(screen.getByText('本地运行，数据归你')).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/WhyStoryForge.test.tsx
```

Expected: FAIL — `WhyStoryForge` not found.

- [ ] **Step 3: 实现组件**

创建 `landing/src/components/WhyStoryForge.tsx`：

```tsx
import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { SectionTitle } from './SectionTitle';

const advantages = [
  {
    title: '长上下文不丢约束',
    description:
      'Context Prioritizer 按关键程度排序系统提示，并在结尾双重锚定，缓解长上下文中的「Lost in the Middle」，让角色、伏笔、世界观设定始终被 AI 记住。',
  },
  {
    title: '稳定压倒灵感',
    description:
      '四级错误分类（Fatal / Retry / Degraded / UserAction）+ 自重复 8% 重试闸门 + 场景优先架构，让 AI 在长篇幅创作中输出更可控、更少崩溃。',
  },
  {
    title: '本地运行，数据归你',
    description:
      'Windows / macOS / Linux 桌面端本地运行，小说数据留在你的电脑上；开源项目，源代码可在 GitHub 查看与审计。',
  },
];

export function WhyStoryForge() {
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
        label="05"
        title="为什么草苔能 hold 住长篇？"
        description="AI 写长篇不是拼灵感，而是拼系统。草苔用工程化的方式守住一致性、稳定性与数据主权。"
      />

      <motion.div
        initial={reduced ? undefined : 'hidden'}
        whileInView={reduced ? undefined : 'visible'}
        viewport={{ once: true, margin: '-100px' }}
        variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
        className="grid gap-8 md:grid-cols-3"
      >
        {advantages.map((a) => (
          <motion.div
            key={a.title}
            variants={reduced ? undefined : item}
            className="border-t-2 border-cinnabar/20 pt-6"
          >
            <h3 className="mb-3 text-xl text-ink">{a.title}</h3>
            <p className="leading-relaxed text-charcoal">{a.description}</p>
          </motion.div>
        ))}
      </motion.div>
    </section>
  );
}
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/WhyStoryForge.test.tsx
```

Expected: PASS (1 test).

- [ ] **Step 5: 提交**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/WhyStoryForge.tsx landing/src/components/__tests__/WhyStoryForge.test.tsx
git commit -m "feat(landing): add WhyStoryForge advantage section"
```

---

### Task 8: 更新 Features 文案

**Files:**
- Modify: `landing/src/components/Features.tsx`

**Interfaces:**
- Consumes: `SectionTitle`, `FeatureFrame`, `useReducedMotion`
- Produces: `<Features>`

- [ ] **Step 1: 重写 Features 文案**

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
          label="06"
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

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/Features.tsx
git commit -m "feat(landing): update Features copy for v3"
```

---

### Task 9: 更新 QuickStart 文案并修复测试

**Files:**
- Modify: `landing/src/components/QuickStart.tsx`
- Modify: `landing/src/components/__tests__/QuickStart.test.tsx`

**Interfaces:**
- Consumes: `SectionTitle`, `StepCard`, `useReducedMotion`
- Produces: `<QuickStart>`

- [ ] **Step 1: 更新测试断言**

编辑 `landing/src/components/__tests__/QuickStart.test.tsx`：

```tsx
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { QuickStart } from '../QuickStart';

describe('QuickStart', () => {
  it('renders three steps', () => {
    render(<QuickStart />);
    expect(screen.getByText('下载安装桌面版')).toBeInTheDocument();
    expect(screen.getByText('用 Genesis 创建故事')).toBeInTheDocument();
    expect(screen.getByText('进入幕前，写下第一段')).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 重写 QuickStart 文案**

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
        label="07"
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

- [ ] **Step 3: 运行测试确认通过**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run src/components/__tests__/QuickStart.test.tsx
```

Expected: PASS (1 test).

- [ ] **Step 4: 提交**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/QuickStart.tsx landing/src/components/__tests__/QuickStart.test.tsx
git commit -m "feat(landing): update QuickStart copy and fix test"
```

---

### Task 10: 更新 Footer 加入 LOGO

**Files:**
- Modify: `landing/src/components/Footer.tsx`

**Interfaces:**
- Produces: `<Footer>`

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

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/components/Footer.tsx
git commit -m "feat(landing): add logo to Footer"
```

---

### Task 11: 更新 App.tsx 组装新结构

**Files:**
- Modify: `landing/src/App.tsx`
- Delete: `landing/src/components/Solution.tsx`（如仍存在）

**Interfaces:**
- Consumes: `Navbar`, `Hero`, `ValueProp`, `PainPoints`, `BackstageFrontstage`, `Genesis`, `TimeSliced`, `WhyStoryForge`, `Features`, `QuickStart`, `DownloadCTA`, `Footer`
- Produces: 落地页整体布局

- [ ] **Step 1: 删除旧 Solution 组件（如存在）**

```bash
rm -f /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing/src/components/Solution.tsx
```

- [ ] **Step 2: 重写 App.tsx**

编辑 `landing/src/App.tsx`：

```tsx
import { Navbar } from './components/Navbar';
import { Hero } from './components/Hero';
import { ValueProp } from './components/ValueProp';
import { PainPoints } from './components/PainPoints';
import { BackstageFrontstage } from './components/BackstageFrontstage';
import { Genesis } from './components/Genesis';
import { TimeSliced } from './components/TimeSliced';
import { WhyStoryForge } from './components/WhyStoryForge';
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
        <PainPoints />
        <BackstageFrontstage />
        <Genesis />
        <TimeSliced />
        <WhyStoryForge />
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

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/src/App.tsx
git rm -f landing/src/components/Solution.tsx || true
git commit -m "feat(landing): assemble v3 layout in App.tsx"
```

---

### Task 12: 全局验证

**Files:**
- 所有 `landing/src` 下已修改文件

**Interfaces:**
- Produces: 通过类型检查、测试与构建的最终版本

- [ ] **Step 1: 类型检查**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx tsc --noEmit
```

Expected: 零错误。

- [ ] **Step 2: 运行测试**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npx vitest run
```

Expected: 全部通过。

- [ ] **Step 3: 构建生产版本**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
npm run build
```

Expected: `landing/dist/` 生成成功。

- [ ] **Step 4: 提交**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/dist || true
git commit -m "chore(landing): verify tsc, tests and build" || true
```

---

### Task 13: 部署到 ai.91z.net

**Files:**
- `landing/dist/`
- `landing/scripts/deploy.js`

**Interfaces:**
- Produces: ai.91z.net 上的更新页面

- [ ] **Step 1: 设置环境变量并执行部署脚本**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page/landing
FTP_HOST=23.106.154.76 FTP_PORT=14121 FTP_USER=gaoge FTP_PASS=88152353 npm run deploy
```

Expected: 部署脚本输出上传进度，无错误。

- [ ] **Step 2: 浏览器验证**

打开 `https://ai.91z.net`（或用户提供的域名），强制刷新页面，确认：
- LOGO 显示正常。
- Hero 文案为 v3 版本。
- 各区块顺序正确。
- 无白屏或 404。

注意：此前从本环境 `curl ai.91z.net` 可能超时，以浏览器实际显示为准。

- [ ] **Step 3: 提交/记录部署**

```bash
cd /Users/yuzaimu/projects/StoryForge/.worktrees/feat-landing-page
git add landing/dist || true
git commit -m "deploy(landing): deploy v3 to ai.91z.net" || true
```

---

## Self-Review

**Spec coverage:**
- Hero / ValueProp / PainPoints / BackstageFrontstage / Genesis / TimeSliced / WhyStoryForge / Features / QuickStart / Footer / App.tsx 均已对应设计文档任务。
- 部署到 ai.91z.net 已包含凭据与验证步骤。

**Placeholder scan:**
- 无 TBD、TODO 或「后续补充」占位符。
- 每个任务均包含完整组件/测试代码与预期命令输出。

**Type consistency：**
- 所有组件沿用现有 `useReducedMotion`、`SectionTitle`、`StepCard`、`FeatureFrame`、`InkButton` 接口，无新增 props 或签名变更。
- `App.tsx` 中新增的 `WhyStoryForge` 为默认导出函数组件，与其他组件一致。
