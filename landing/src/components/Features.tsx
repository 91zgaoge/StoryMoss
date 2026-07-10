import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { BookOpen, Users, Globe, Sparkles } from 'lucide-react';

const features = [
  {
    icon: BookOpen,
    title: '故事与场景管理',
    description:
      '把一本小说拆成可管理的故事、章节、场景。每个场景都有出场角色、地点、状态，降低“写一章”的心理压力。',
  },
  {
    icon: Users,
    title: '角色与世界观',
    description:
      '系统化人设、关系网络、知识图谱。AI 在续写时严格遵循设定，避免“角色崩坏”和“吃书”。',
  },
  {
    icon: Sparkles,
    title: 'AI 续写与润色',
    description:
      '底部输入栏发指令：“续写下一段”“改得更紧张”“加入意外转折”。AI 随行辅助，但创作主权始终在你。',
  },
  {
    icon: Globe,
    title: '拆书与分析',
    description:
      '上传参考小说，AI 自动分析整体结构、章节节奏、角色出场频率，把“凭感觉写”变成“有参照地写”。',
  },
];

export function Features() {
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
    <section id="features" className="mx-auto max-w-[1100px] px-6 py-[120px] md:py-[160px]">
      <motion.div
        initial={reduced ? undefined : 'hidden'}
        whileInView={reduced ? undefined : 'visible'}
        viewport={{ once: true, margin: '-100px' }}
        variants={reduced ? undefined : item}
        className="mb-16 text-center"
      >
        <h2 className="mb-4 text-[32px] tracking-[-0.015em] text-ink md:text-[40px]">
          一套完整的创作系统
        </h2>
        <p className="mx-auto max-w-[640px] text-lg text-charcoal">
          从灵感、规划、写作到分析，草苔把长篇小说创作的每个环节都装进了工作台。
        </p>
      </motion.div>

      <div className="space-y-24 md:space-y-32">
        {features.map((f, idx) => {
          const Icon = f.icon;
          const isReversed = idx % 2 === 1;

          return (
            <motion.div
              key={f.title}
              initial={reduced ? undefined : 'hidden'}
              whileInView={reduced ? undefined : 'visible'}
              viewport={{ once: true, margin: '-100px' }}
              variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.12 } } }}
              className={`grid items-center gap-10 md:grid-cols-2 ${isReversed ? 'md:flex-row-reverse' : ''}`}
            >
              <motion.div
                variants={reduced ? undefined : item}
                className={isReversed ? 'md:order-2' : ''}
              >
                <div className="mb-4 inline-flex h-12 w-12 items-center justify-center rounded-lg bg-terracotta-soft text-terracotta">
                  <Icon size={24} />
                </div>
                <h3 className="mb-3 text-2xl text-ink md:text-3xl">{f.title}</h3>
                <p className="max-w-[480px] text-lg leading-relaxed text-charcoal">
                  {f.description}
                </p>
              </motion.div>

              <motion.div
                variants={reduced ? undefined : item}
                className={`rounded-xl bg-cream p-8 shadow-card ${isReversed ? 'md:order-1' : ''}`}
              >
                <div className="aspect-[4/3] rounded-lg bg-terracotta-soft/50" aria-label={`${f.title} 占位示意图`} />
              </motion.div>
            </motion.div>
          );
        })}
      </div>
    </section>
  );
}
