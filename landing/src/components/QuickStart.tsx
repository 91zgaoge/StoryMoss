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
    title: '创建你的第一个故事',
    description: '用 AI 创建故事框架，或手动填写标题与简介，从零开始。',
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
        label="05"
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
