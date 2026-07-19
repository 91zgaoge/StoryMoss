import { useRef } from "react";
import { motion, useScroll, useTransform } from "framer-motion";
import type { MotionValue } from "framer-motion";
import { useReducedMotion } from "../hooks/useReducedMotion";

/** 孢子（CSS 驱动的浮游微粒，transform/opacity only） */
function Spores() {
  const spores = [
    { cx: 120, cy: 220, r: 2.5, d: 9, delay: 0 },
    { cx: 260, cy: 180, r: 2, d: 11, delay: 1.2 },
    { cx: 420, cy: 240, r: 2.2, d: 8, delay: 2.4 },
    { cx: 560, cy: 150, r: 1.8, d: 12, delay: 0.6 },
    { cx: 720, cy: 200, r: 2.6, d: 10, delay: 3.1 },
    { cx: 880, cy: 160, r: 2, d: 9.5, delay: 1.8 },
    { cx: 1040, cy: 230, r: 2.4, d: 11.5, delay: 0.9 },
    { cx: 1180, cy: 170, r: 1.9, d: 8.5, delay: 2.8 },
    { cx: 1320, cy: 210, r: 2.3, d: 10.5, delay: 1.5 },
  ];
  return (
    <g>
      {spores.map((s, i) => (
        <circle
          key={i}
          cx={s.cx}
          cy={s.cy}
          r={s.r}
          fill="oklch(0.86 0.08 148)"
          className="spore-drift"
          style={{
            ["--spore-duration" as string]: `${s.d}s`,
            ["--spore-delay" as string]: `${s.delay}s`,
          }}
        />
      ))}
    </g>
  );
}

/** 一丛苔：以原点为根的小椭圆簇 */
function Clump({
  progress,
  from,
  to,
  items,
  fill,
}: {
  progress: MotionValue<number>;
  from: number;
  to: number;
  items: Array<[number, number, number, number, number]>; // cx, cy, rx, ry, rotate
  fill: string;
}) {
  const scaleY = useTransform(progress, [from, to], [0.05, 1]);
  const opacity = useTransform(progress, [from, from + 0.04], [0, 1]);
  return (
    <motion.g style={{ scaleY, opacity, originX: "50%", originY: "100%" }}>
      {items.map(([cx, cy, rx, ry, rotate], i) => (
        <ellipse
          key={i}
          cx={cx}
          cy={cy}
          rx={rx}
          ry={ry}
          fill={fill}
          transform={`rotate(${rotate} ${cx} ${cy})`}
        />
      ))}
    </motion.g>
  );
}

/**
 * 活苔藓微景观：三层苔丘 + 随滚动生长的苔丛 + 浮游孢子 + 雾光。
 * 生长由整页滚动进度驱动（前 ~25% 滚动完成生长）；reduced-motion 直接呈现成景。
 */
export function MossScape() {
  const ref = useRef<HTMLDivElement>(null);
  const reduced = useReducedMotion();
  const { scrollYProgress } = useScroll();
  const progress = reduced
    ? ({ get: () => 1 } as unknown as MotionValue<number>)
    : scrollYProgress;

  const clumpA: Array<[number, number, number, number, number]> = [
    [150, 348, 26, 14, -8],
    [188, 352, 20, 11, 5],
    [120, 356, 18, 10, 3],
    [222, 356, 15, 8, -4],
  ];
  const clumpB: Array<[number, number, number, number, number]> = [
    [620, 330, 30, 16, 6],
    [668, 336, 22, 12, -6],
    [580, 342, 18, 10, 2],
    [712, 344, 16, 9, 7],
  ];
  const clumpC: Array<[number, number, number, number, number]> = [
    [1090, 344, 28, 15, -5],
    [1140, 350, 20, 11, 8],
    [1048, 354, 17, 9, -2],
    [1188, 356, 14, 8, 4],
  ];

  return (
    <div
      ref={ref}
      className="pointer-events-none absolute inset-x-0 bottom-0"
      aria-hidden="true"
    >
      {/* 雾光 */}
      <div className="mist-drift absolute -top-24 left-[8%] h-56 w-96 rounded-full bg-moss opacity-[0.05] blur-3xl" />
      <div className="mist-drift-slow absolute -top-16 right-[12%] h-48 w-80 rounded-full bg-moss-soft opacity-[0.04] blur-3xl" />
      <svg
        viewBox="0 0 1440 420"
        preserveAspectRatio="xMidYMax slice"
        className="block h-[300px] w-full md:h-[420px]"
      >
        {/* 远山（墨绿渐深三层） */}
        <path
          d="M0,300 C180,240 320,260 480,300 C640,340 780,250 960,290 C1140,330 1290,270 1440,300 L1440,420 L0,420 Z"
          fill="oklch(0.24 0.03 158)"
        />
        {/* 中丘 */}
        <path
          d="M0,340 C160,310 300,320 460,345 C620,370 760,320 940,340 C1120,360 1290,330 1440,350 L1440,420 L0,420 Z"
          fill="oklch(0.30 0.045 155)"
        />
        {/* 近丘 */}
        <path
          d="M0,380 C200,355 380,365 560,385 C740,405 900,365 1100,382 C1240,394 1350,378 1440,386 L1440,420 L0,420 Z"
          fill="oklch(0.36 0.06 152)"
        />
        {/* 三丛苔（滚动生长） */}
        <Clump
          progress={progress}
          from={0.02}
          to={0.18}
          items={clumpA}
          fill="oklch(0.58 0.11 152)"
        />
        <Clump
          progress={progress}
          from={0.07}
          to={0.22}
          items={clumpB}
          fill="oklch(0.66 0.12 150)"
        />
        <Clump
          progress={progress}
          from={0.12}
          to={0.26}
          items={clumpC}
          fill="oklch(0.58 0.11 152)"
        />
        {/* 孢子 */}
        <Spores />
      </svg>
    </div>
  );
}
