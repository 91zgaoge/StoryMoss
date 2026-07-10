import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { SectionTitle } from './SectionTitle';
import { FeatureFrame } from './FeatureFrame';

const features = [
  {
    title: '故事与场景管理',
    description:
      '把一本小说拆成可管理的故事、章节、场景。每个场景都有出场角色、地点、状态，降低“写一章”的心理压力。',
    image: '/screenshots/stories.png',
    alt: '故事与场景管理界面',
  },
  {
    title: '角色与世界观',
    description:
      '系统化人设、关系网络、知识图谱。AI 在续写时严格遵循设定，避免“角色崩坏”和“吃书”。',
    image: '/screenshots/characters.png',
    alt: '角色与世界观管理界面',
  },
  {
    title: 'AI 续写与润色',
    description:
      '底部输入栏发指令：“续写下一段”“改得更紧张”“加入意外转折”。AI 随行辅助，但创作主权始终在你。',
    image: '/screenshots/frontstage.png',
    alt: '幕前沉浸式写作界面',
  },
  {
    title: '拆书与叙事分析',
    description:
      '上传参考小说，AI 自动分析整体结构、章节节奏、角色出场频率，把“凭感觉写”变成“有参照地写”。',
    image: '/screenshots/book-deconstruction.png',
    alt: '拆书与叙事分析界面',
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
    <section id="features" className="mx-auto max-w-[980px] px-6 py-[100px] md:py-[160px]">
      <SectionTitle
        label="03"
        title="从第一行字到完本"
        description="从灵感、规划、写作到分析，草苔把长篇小说创作的每个环节都装进了工作台。"
      />

      <div className="space-y-20 md:space-y-28">
        {features.map((f, idx) => {
          const isReversed = idx % 2 === 1;

          return (
            <motion.div
              key={f.title}
              initial={reduced ? undefined : 'hidden'}
              whileInView={reduced ? undefined : 'visible'}
              viewport={{ once: true, margin: '-100px' }}
              variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
              className="grid items-center gap-10 md:grid-cols-2"
            >
              <motion.div
                variants={reduced ? undefined : item}
                className={isReversed ? 'md:order-2' : ''}
              >
                <h3 className="mb-3 text-2xl text-ink md:text-3xl">{f.title}</h3>
                <p className="max-w-[480px] text-base leading-relaxed text-charcoal md:text-lg">
                  {f.description}
                </p>
              </motion.div>

              <motion.div
                variants={reduced ? undefined : item}
                className={isReversed ? 'md:order-1' : ''}
              >
                <FeatureFrame src={f.image} alt={f.alt} />
              </motion.div>
            </motion.div>
          );
        })}
      </div>
    </section>
  );
}
