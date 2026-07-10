import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { SectionTitle } from './SectionTitle';

export function Solution() {
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
    <section id="approach" className="border-y border-ink-line bg-cream py-[100px] md:py-[160px]">
      <div className="mx-auto max-w-[980px] px-6">
        <SectionTitle
          label="02"
          title="两个空间，各尽其职"
          description="把创作拆成两个空间：幕后把要素结构化管好，幕前让你专注写字。"
        />

        <div className="grid gap-8 md:grid-cols-2 md:divide-x md:divide-ink-line">
          <motion.div
            initial={reduced ? undefined : 'hidden'}
            whileInView={reduced ? undefined : 'visible'}
            viewport={{ once: true, margin: '-100px' }}
            variants={reduced ? undefined : item}
            className="md:pr-10"
          >
            <h3 className="mb-3 text-2xl text-ink">幕后工作室</h3>
            <p className="leading-relaxed text-charcoal">
              管理故事、角色、场景、世界观、知识图谱。AI 帮你生成人设、追踪伏笔、分析叙事结构，让设定不崩。
            </p>
          </motion.div>

          <motion.div
            initial={reduced ? undefined : 'hidden'}
            whileInView={reduced ? undefined : 'visible'}
            viewport={{ once: true, margin: '-100px' }}
            variants={reduced ? undefined : item}
            className="md:pl-10"
          >
            <h3 className="mb-3 text-2xl text-ink">幕前写作台</h3>
            <p className="leading-relaxed text-charcoal">
              极简、全屏、自动保存。底部输入栏随时调用 AI 续写、润色、改紧张感，不打断心流。
            </p>
          </motion.div>
        </div>
      </div>
    </section>
  );
}
