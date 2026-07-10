import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';

export function PainPoints() {
  const reduced = useReducedMotion();

  const variants = {
    hidden: { opacity: 0, y: 16 },
    visible: {
      opacity: 1,
      y: 0,
      transition: { duration: 0.6, ease: [0.16, 1, 0.3, 1] as const },
    },
  };

  const pains = [
    '角色写着写着就崩了',
    '伏笔埋了却忘了回收',
    '设定越写越自相矛盾',
  ];

  return (
    <section className="mx-auto max-w-[1100px] px-6 py-[120px] md:py-[160px]">
      <div className="grid gap-12 md:grid-cols-12 md:gap-8">
        <motion.div
          initial={reduced ? undefined : 'hidden'}
          whileInView={reduced ? undefined : 'visible'}
          viewport={{ once: true, margin: '-100px' }}
          variants={reduced ? undefined : variants}
          className="md:col-span-7"
        >
          <h2 className="mb-6 text-[32px] leading-tight tracking-[-0.015em] text-ink md:text-[40px]">
            写长篇，最怕的不是没灵感
          </h2>
          <p className="max-w-[560px] text-lg leading-relaxed text-charcoal">
            灵感会再来，但角色关系、伏笔线索、世界观设定一旦失控，修改成本就会指数级上升。
            大多数作者不是缺想法，而是缺一个让想法不散架的系统。
          </p>
        </motion.div>

        <motion.aside
          initial={reduced ? undefined : 'hidden'}
          whileInView={reduced ? undefined : 'visible'}
          viewport={{ once: true, margin: '-100px' }}
          variants={
            reduced
              ? undefined
              : {
                  ...variants,
                  visible: {
                    ...variants.visible,
                    transition: { ...variants.visible.transition, delay: 0.15 },
                  },
                }
          }
          className="md:col-span-5"
          aria-label="常见创作痛点"
        >
          <div className="mb-4 h-[2px] w-10 bg-gold" />
          <ul className="space-y-4 font-display text-xl text-ink">
            {pains.map((pain) => (
              <li key={pain} className="flex items-baseline gap-3">
                <span className="text-gold">·</span>
                {pain}
              </li>
            ))}
          </ul>
        </motion.aside>
      </div>
    </section>
  );
}
