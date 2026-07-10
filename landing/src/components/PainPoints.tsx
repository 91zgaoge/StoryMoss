import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { SectionTitle } from './SectionTitle';

const pains = [
  { title: '角色写着写着崩了', description: '人设越写越散，前后言行不一致。' },
  { title: '伏笔埋了忘了回收', description: '前期线索后期无踪，读者白期待。' },
  { title: '世界观越写越矛盾', description: '设定越来越多，互相冲突难以自洽。' },
];

export function PainPoints() {
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
    <section className="mx-auto max-w-[980px] px-6 py-[100px] md:py-[160px]">
      <SectionTitle
        label="01"
        title="写到中途，往往毁于细节"
        description="灵感会再来，但角色关系、伏笔线索、世界观设定一旦失控，修改成本就会指数级上升。"
      />

      <motion.div
        initial={reduced ? undefined : 'hidden'}
        whileInView={reduced ? undefined : 'visible'}
        viewport={{ once: true, margin: '-100px' }}
        variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
        className="grid gap-6 md:grid-cols-3"
      >
        {pains.map((pain) => (
          <motion.div
            key={pain.title}
            variants={reduced ? undefined : item}
            className="group border-b border-ink-line pb-6 transition-colors duration-200 hover:border-cinnabar"
          >
            <h3 className="mb-3 text-xl text-ink">{pain.title}</h3>
            <p className="text-charcoal">{pain.description}</p>
          </motion.div>
        ))}
      </motion.div>
    </section>
  );
}
