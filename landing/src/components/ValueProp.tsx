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
