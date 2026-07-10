import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { SectionTitle } from './SectionTitle';
import { FeatureFrame } from './FeatureFrame';

export function BackstageFrontstage() {
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
          label="01"
          title="两个空间，各尽其职"
          description="把规划与写作拆成两个空间：幕后用 AI 系统化管好创作资产，幕前只留你和文字。"
        />

        <div className="space-y-16 md:space-y-24">
          <motion.div
            initial={reduced ? undefined : 'hidden'}
            whileInView={reduced ? undefined : 'visible'}
            viewport={{ once: true, margin: '-100px' }}
            variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
            className="grid items-center gap-10 md:grid-cols-2"
          >
            <motion.div variants={reduced ? undefined : item}>
              <h3 className="mb-4 text-2xl text-ink md:text-3xl">幕后工作室</h3>
              <ul className="space-y-3 text-charcoal">
                <li>管理故事、角色、场景、世界观</li>
                <li>知识图谱可视化人物、地点、事件关系</li>
                <li>伏笔看板追踪线索的埋下与回收</li>
                <li>AI 模型、提示词、创作方法论配置</li>
              </ul>
            </motion.div>
            <motion.div variants={reduced ? undefined : item}>
              <FeatureFrame src="/screenshots/dashboard.png" alt="幕后工作室仪表盘" />
            </motion.div>
          </motion.div>

          <motion.div
            initial={reduced ? undefined : 'hidden'}
            whileInView={reduced ? undefined : 'visible'}
            viewport={{ once: true, margin: '-100px' }}
            variants={reduced ? undefined : { visible: { transition: { staggerChildren: 0.1 } } }}
            className="grid items-center gap-10 md:grid-cols-2"
          >
            <motion.div variants={reduced ? undefined : item} className="md:order-2">
              <h3 className="mb-4 text-2xl text-ink md:text-3xl">幕前写作台</h3>
              <ul className="space-y-3 text-charcoal">
                <li>极简、全屏、自动保存，无干扰码字环境</li>
                <li>底部输入栏随时调用 AI 续写、润色、改紧张感</li>
                <li>文思模式切换 AI 介入程度，被动或主动辅助</li>
                <li>创作主权在你，AI 只在需要时随行</li>
              </ul>
            </motion.div>
            <motion.div variants={reduced ? undefined : item} className="md:order-1">
              <FeatureFrame src="/screenshots/frontstage.png" alt="幕前沉浸式写作台" />
            </motion.div>
          </motion.div>
        </div>
      </div>
    </section>
  );
}
