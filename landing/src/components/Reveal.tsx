import { motion } from "framer-motion";
import type { ReactNode } from "react";
import { useReducedMotion } from "../hooks/useReducedMotion";

/**
 * 章节错落浮现容器：进入视口时子元素按 ~100ms 间隔
 * opacity 0→1 / translateY(12px)→0 / blur(4px)→0。
 */
export function Reveal({
  children,
  className = "",
}: {
  children: ReactNode;
  className?: string;
}) {
  const reduced = useReducedMotion();
  return (
    <motion.div
      className={className}
      initial={reduced ? undefined : { opacity: 0, y: 12, filter: "blur(4px)" }}
      whileInView={
        reduced ? undefined : { opacity: 1, y: 0, filter: "blur(0px)" }
      }
      viewport={{ once: true, margin: "-80px" }}
      transition={{ duration: 0.6, ease: [0.16, 1, 0.3, 1] }}
    >
      {children}
    </motion.div>
  );
}

export function SectionHeader({
  kicker,
  title,
  lead,
}: {
  kicker: string;
  title: string;
  lead?: string;
}) {
  return (
    <Reveal className="mb-12 max-w-[640px] md:mb-16">
      <p className="mb-3 text-xs tracking-[0.2em] text-moss">{kicker}</p>
      <h2 className="text-balance text-3xl leading-[1.15] tracking-mid text-paper md:text-[44px]">
        {title}
      </h2>
      {lead && (
        <p className="text-pretty mt-4 leading-relaxed text-mist">{lead}</p>
      )}
    </Reveal>
  );
}
