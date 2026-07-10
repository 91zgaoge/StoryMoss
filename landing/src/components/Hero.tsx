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
      transition: { duration: 0.6, ease: [0.16, 1, 0.3, 1] as const },
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
