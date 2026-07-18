import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { DownloadButton } from './DownloadButton';
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
        className="max-w-[760px]"
      >
        <motion.div variants={reduced ? undefined : child} className="mb-6 flex justify-center">
          <img
            src="/logo.png"
            alt="StoryMoss 草苔"
            className="h-20 w-20 object-contain"
          />
        </motion.div>

        <motion.h1
          variants={reduced ? undefined : child}
          className="mb-6 text-[40px] leading-[1.12] tracking-[-0.02em] text-ink md:text-[64px]"
        >
          写长篇小说，<br className="hidden md:block" />
          终于有一间自己的工作室
        </motion.h1>

        <motion.p
          variants={reduced ? undefined : child}
          className="mx-auto mb-4 max-w-[640px] text-base leading-relaxed text-charcoal md:text-lg"
        >
          草苔 StoryMoss 是专为长篇小说作者打造的 AI 创作系统：幕后用知识图谱、伏笔看板、角色与世界观把创作资产管清楚；幕前给你无干扰的写作台，AI 在需要时续写、润色、审校，创作主权始终在你。
        </motion.p>

        <motion.p
          variants={reduced ? undefined : child}
          className="mx-auto mb-10 max-w-[600px] text-sm text-stone"
        >
          v0.29.0 · 本地运行 · 开源可审计 · Windows / macOS / Linux
        </motion.p>

        <motion.div
          variants={reduced ? undefined : child}
          className="flex flex-col items-center justify-center gap-4 sm:flex-row"
        >
          <DownloadButton variant="primary" />
          <InkButton
            as="a"
            href="#approach"
            variant="secondary"
            className="group"
          >
            了解草苔如何工作
            <ChevronDown
              className="ml-1 inline-block transition-transform group-hover:translate-y-0.5"
              size={16}
            />
          </InkButton>
        </motion.div>
      </motion.div>
    </section>
  );
}
