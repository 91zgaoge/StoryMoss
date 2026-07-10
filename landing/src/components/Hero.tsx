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
          <InkButton
            as="a"
            href="https://github.com/91zgaoge/StoryForge/releases/latest"
            target="_blank"
            rel="noreferrer"
            variant="primary"
          >
            免费下载桌面版
          </InkButton>
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
