import { motion } from "framer-motion";
import { useReducedMotion } from "../hooks/useReducedMotion";
import { DownloadButton } from "./DownloadButton";
import { InkButton } from "./InkButton";
import { MossScape } from "./MossScape";
import { ChevronDown } from "lucide-react";

export function Hero() {
  const reduced = useReducedMotion();

  const container = {
    hidden: {},
    visible: { transition: { staggerChildren: 0.1 } },
  };

  const child = {
    hidden: { opacity: 0, y: 12, filter: "blur(4px)" },
    visible: {
      opacity: 1,
      y: 0,
      filter: "blur(0px)",
      transition: { duration: 0.6, ease: [0.16, 1, 0.3, 1] as const },
    },
  };

  return (
    <section className="relative flex min-h-screen flex-col justify-center overflow-hidden px-6 pt-[72px]">
      <div className="moss-texture absolute inset-0" aria-hidden="true" />

      <div className="relative z-10 mx-auto w-full max-w-[1080px]">
        <motion.div
          variants={reduced ? undefined : container}
          initial="hidden"
          animate="visible"
          className="max-w-[720px]"
        >
          <motion.div variants={reduced ? undefined : child} className="mb-8">
            <span className="surface-2 inline-flex items-center gap-2 rounded-full border border-subtle px-4 py-1.5 text-xs tracking-wide text-moss-soft">
              <span
                className="inline-block h-1.5 w-1.5 rounded-full bg-moss"
                aria-hidden="true"
              />
              v0.30.0 · 多代理创作框架
            </span>
          </motion.div>

          <motion.h1
            variants={reduced ? undefined : child}
            className="text-balance mb-8 text-[52px] leading-[1.08] tracking-display text-paper md:text-[88px]"
          >
            让故事
            <br />
            自己生长
          </motion.h1>

          <motion.p
            variants={reduced ? undefined : child}
            className="text-pretty mb-6 max-w-[560px] text-base leading-relaxed text-mist md:text-lg"
          >
            草苔 StoryMoss
            是一间有三个创作者的工作室：主创执笔、管理备料、编辑审计把关。AI
            多代理并行创作，质量门逐章验收——你只管讲故事。
          </motion.p>

          <motion.p
            variants={reduced ? undefined : child}
            className="mb-10 max-w-[560px] text-sm text-dim"
          >
            本地运行 · 开源可审计 · Windows / macOS / Linux
          </motion.p>

          <motion.div
            variants={reduced ? undefined : child}
            className="flex flex-col items-start gap-4 sm:flex-row sm:items-center"
          >
            <DownloadButton variant="primary" />
            <InkButton
              as="a"
              href="#trio"
              variant="secondary"
              className="group"
            >
              看看三个创作者
              <ChevronDown
                className="ml-1 inline-block transition-transform group-hover:translate-y-0.5"
                size={16}
              />
            </InkButton>
          </motion.div>
        </motion.div>
      </div>

      <MossScape />
    </section>
  );
}
