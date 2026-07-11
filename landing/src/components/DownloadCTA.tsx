import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { DownloadButton } from './DownloadButton';

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
          <DownloadButton
            variant="primary"
            className="px-10 py-4 text-base"
          />
        </motion.div>
        <motion.p variants={reduced ? undefined : item} className="mt-4 text-sm text-charcoal">
          开源项目，源代码可在 GitHub 查看
        </motion.p>
      </motion.div>
    </section>
  );
}
