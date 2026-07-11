import { motion } from 'framer-motion';
import { useReducedMotion } from '../hooks/useReducedMotion';

export function BrandSlogan() {
  const reduced = useReducedMotion();

  return (
    <section className="relative overflow-hidden border-y border-ink-line bg-parchment py-[100px] md:py-[160px]">
      <div className="paper-texture absolute inset-0" />
      <div className="absolute left-1/2 top-1/2 h-[480px] w-[480px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-ink-wash blur-3xl" />

      <motion.div
        initial={reduced ? undefined : { opacity: 0, y: 20 }}
        whileInView={reduced ? undefined : { opacity: 1, y: 0 }}
        viewport={{ once: true, margin: '-100px' }}
        transition={{ duration: 0.8, ease: [0.16, 1, 0.3, 1] }}
        className="relative mx-auto max-w-[720px] px-6 text-center"
      >
        <p className="mb-12 font-display text-[28px] leading-snug tracking-[-0.02em] text-ink md:text-[44px]">
          故事如苔，沉静漫长。
        </p>

        <div className="space-y-6 text-balance text-base leading-[1.8] text-charcoal md:text-lg">
          <p>
            伟大的故事从不是凭空蹦出来的巨石，而是无数文字像思想的孢子，在记忆与历史的角落里，悄无声息地附着、渗透、蔓延。
          </p>

          <p>
            <strong className="font-medium text-ink">StoryMoss (草苔)</strong>，为你守护这片心流的湿地。幕后，我们把庞杂的世界、角色与场景、剧情妥善打理；幕前，唯有你与呼吸般的 AI 相润相随。
          </p>

          <p className="text-ink">
            俯身凝视，你笔下的大千世界，漫长出绿意盎然的未来。
          </p>
        </div>
      </motion.div>
    </section>
  );
}
