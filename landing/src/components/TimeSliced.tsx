import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { SectionTitle } from './SectionTitle';
import { StepCard } from './StepCard';

const steps = [
  {
    number: '01',
    title: '写作时刻',
    description: '秒出正文。只带最小必要约束，让灵感不被流程卡住。',
  },
  {
    number: '02',
    title: '审计时刻',
    description: '后台自动审校，问题以标注形式回流编辑器，当场处理小债。',
  },
  {
    number: '03',
    title: '洞察时刻',
    description: '定期产出叙事健康度报告，发现节奏与结构问题。',
  },
];

export function TimeSliced() {
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
    <section className="border-y border-ink-line bg-cream py-[100px] md:py-[160px]">
      <div className="mx-auto max-w-[980px] px-6">
        <SectionTitle
          label="04"
          title="写得快，也审得深"
          description="分时介入架构把“写”和“审”拆成三条时间线，不再让质量与速度互相拖累。"
        />

        <motion.div
          initial={reduced ? undefined : 'hidden'}
          whileInView={reduced ? undefined : 'visible'}
          viewport={{ once: true, margin: '-100px' }}
          variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
          className="relative grid gap-10 md:grid-cols-3"
        >
          <div className="absolute top-[42px] left-0 hidden h-px w-full bg-ink-line md:block" />
          {steps.map((s) => (
            <motion.div key={s.number} variants={reduced ? undefined : item}>
              <StepCard number={s.number} title={s.title} description={s.description} />
            </motion.div>
          ))}
        </motion.div>
      </div>
    </section>
  );
}
