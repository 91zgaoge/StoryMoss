import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';

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
    <section className="bg-terracotta-soft py-[120px] md:py-[160px]">
      <div className="mx-auto max-w-[1100px] px-6">
        <motion.div
          initial={reduced ? undefined : 'hidden'}
          whileInView={reduced ? undefined : 'visible'}
          viewport={{ once: true, margin: '-100px' }}
          variants={reduced ? undefined : item}
          className="mb-16 text-center"
        >
          <h2 className="mb-4 text-[32px] tracking-[-0.015em] text-ink md:text-[40px]">
            写得快，也审得深
          </h2>
          <p className="mx-auto max-w-[640px] text-lg text-charcoal">
            分时介入架构把“写”和“审”拆成三条时间线，不再让质量与速度互相拖累。
          </p>
        </motion.div>

        <motion.div
          initial={reduced ? undefined : 'hidden'}
          whileInView={reduced ? undefined : 'visible'}
          viewport={{ once: true, margin: '-100px' }}
          variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.12 } } }}
          className="grid gap-8 md:grid-cols-3"
        >
          {steps.map((s) => (
            <motion.div key={s.number} variants={reduced ? undefined : item} className="text-center md:text-left">
              <span className="mb-3 block font-display text-4xl text-gold">{s.number}</span>
              <h3 className="mb-2 text-xl text-ink">{s.title}</h3>
              <p className="leading-relaxed text-charcoal">{s.description}</p>
            </motion.div>
          ))}
        </motion.div>
      </div>
    </section>
  );
}
