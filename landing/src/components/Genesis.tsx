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
    description: '自动进入幕前写作台，第一章正文已就绪，可立即开始修改与续写。',
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
        label="02"
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
