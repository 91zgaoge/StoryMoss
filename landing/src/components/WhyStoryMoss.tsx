import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { SectionTitle } from './SectionTitle';

const advantages = [
  {
    title: '长上下文不丢约束',
    description:
      'Context Prioritizer 按关键程度排序系统提示，并在结尾双重锚定，缓解长上下文中的「Lost in the Middle」，让角色、伏笔、世界观设定始终被 AI 记住。',
  },
  {
    title: '稳定压倒灵感',
    description:
      '四级错误分类（Fatal / Retry / Degraded / UserAction）+ 自重复 8% 重试闸门 + 场景优先架构，让 AI 在长篇幅创作中输出更可控、更少崩溃。',
  },
  {
    title: '本地运行，数据归你',
    description:
      'Windows / macOS / Linux 桌面端本地运行，小说数据留在你的电脑上；开源项目，源代码可在 GitHub 查看与审计。',
  },
];

export function WhyStoryMoss() {
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
        label="05"
        title="为什么草苔能 hold 住长篇？"
        description="AI 写长篇不是拼灵感，而是拼系统。草苔用工程化的方式守住一致性、稳定性与数据主权。"
      />

      <motion.div
        initial={reduced ? undefined : 'hidden'}
        whileInView={reduced ? undefined : 'visible'}
        viewport={{ once: true, margin: '-100px' }}
        variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
        className="grid gap-8 md:grid-cols-3"
      >
        {advantages.map((a) => (
          <motion.div
            key={a.title}
            variants={reduced ? undefined : item}
            className="border-t-2 border-cinnabar/20 pt-6"
          >
            <h3 className="mb-3 text-xl text-ink">{a.title}</h3>
            <p className="leading-relaxed text-charcoal">{a.description}</p>
          </motion.div>
        ))}
      </motion.div>
    </section>
  );
}
