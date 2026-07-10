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
          <motion.span variants={reduced ? undefined : child} className="block">
            写长篇，
          </motion.span>
          <motion.span variants={reduced ? undefined : child} className="block">
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
