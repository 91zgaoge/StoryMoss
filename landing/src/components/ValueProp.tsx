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
        草苔是专为长篇小说作者设计的系统工作台：幕后管理故事资产，幕前沉浸式写作，Genesis 把创意变成可执行的创作结构。
      </motion.p>
    </section>
  );
}
