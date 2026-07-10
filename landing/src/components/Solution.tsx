import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';

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
    <section id="approach" className="bg-cream py-[120px] md:py-[160px]">
      <div className="mx-auto max-w-[1100px] px-6">
        <motion.div
          initial={reduced ? undefined : 'hidden'}
          whileInView={reduced ? undefined : 'visible'}
          viewport={{ once: true, margin: '-100px' }}
          variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.12 } } }}
          className="mb-16 text-center"
        >
          <motion.h2
            variants={reduced ? undefined : item}
            className="mb-4 text-[32px] tracking-[-0.015em] text-ink md:text-[40px]"
          >
            幕后规划，幕前写作
          </motion.h2>
          <motion.p variants={reduced ? undefined : item} className="mx-auto max-w-[640px] text-lg text-charcoal">
            把创作拆成两个空间：幕后把要素结构化管好，幕前让你专注写字。
          </motion.p>
        </motion.div>

        <div className="grid gap-8 md:grid-cols-2">
          <motion.div
            initial={reduced ? undefined : 'hidden'}
            whileInView={reduced ? undefined : 'visible'}
            viewport={{ once: true, margin: '-100px' }}
            variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
            className="rounded-xl bg-parchment p-8 shadow-card"
          >
            <motion.h3 variants={reduced ? undefined : item} className="mb-3 text-2xl text-ink">
              幕后工作室
            </motion.h3>
            <motion.p variants={reduced ? undefined : item} className="leading-relaxed text-charcoal">
              管理故事、角色、场景、世界观、知识图谱。AI 帮你生成人设、追踪伏笔、分析叙事结构，让设定不崩。
            </motion.p>
          </motion.div>

          <motion.div
            initial={reduced ? undefined : 'hidden'}
            whileInView={reduced ? undefined : 'visible'}
            viewport={{ once: true, margin: '-100px' }}
            variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1, delayChildren: 0.15 } } }}
            className="rounded-xl bg-parchment p-8 shadow-card"
          >
            <motion.h3 variants={reduced ? undefined : item} className="mb-3 text-2xl text-ink">
              幕前写作台
            </motion.h3>
            <motion.p variants={reduced ? undefined : item} className="leading-relaxed text-charcoal">
              极简、全屏、自动保存。底部输入栏随时调用 AI 续写、润色、改紧张感，不打断心流。
            </motion.p>
          </motion.div>
        </div>
      </div>
    </section>
  );
}
