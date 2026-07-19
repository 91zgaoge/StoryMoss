import { PenLine, Eye, Sprout, Wand2 } from "lucide-react";
import { Reveal, SectionHeader } from "./Reveal";

const STEPS = [
  {
    icon: PenLine,
    title: "你创作",
    desc: "写下的每一章、每一次修改都被看在眼里（本地观察日志，不出你的电脑）。",
  },
  {
    icon: Eye,
    title: "它观察",
    desc: "后台分析器把反复出现的模式沉淀为「模式卡」：什么有效、什么被否决。",
  },
  {
    icon: Sprout,
    title: "你确认",
    desc: "模式攒够置信度、跨故事复现，学习中心把晋升提案交到你手上，一键确认。",
  },
  {
    icon: Wand2,
    title: "成技能",
    desc: "确认后的模式物化为创作技能，下次开写自动生效——工作室随你成长。",
  },
];

export function LoopSection() {
  return (
    <section id="learning" className="relative px-6 py-24 md:py-36">
      <div className="mx-auto max-w-[1080px]">
        <SectionHeader
          kicker="持续学习"
          title="越写，越懂你"
          lead="草苔会把创作过程里反复有效的做法学成技能。不是云端黑箱——观察、模式、技能三层文件都在你本地，每一步晋升都要你点头。"
        />

        <div className="grid gap-5 md:grid-cols-4">
          {STEPS.map((step, i) => (
            <Reveal key={step.title} className="relative">
              <article className="surface-1 h-full rounded-lg border border-subtle p-5">
                <div className="mb-4 flex items-center justify-between">
                  <div className="surface-2 inline-flex h-10 w-10 items-center justify-center rounded-md text-moss">
                    <step.icon size={20} aria-hidden="true" />
                  </div>
                  <span
                    className="font-display text-2xl text-dim"
                    aria-hidden="true"
                  >
                    {String(i + 1).padStart(2, "0")}
                  </span>
                </div>
                <h3 className="mb-2 text-lg text-paper">{step.title}</h3>
                <p className="text-pretty text-sm leading-relaxed text-mist">
                  {step.desc}
                </p>
              </article>
              {i < STEPS.length - 1 && (
                <div
                  className="absolute right-[-14px] top-1/2 hidden h-px w-[18px] border-t border-dashed border-standard md:block"
                  aria-hidden="true"
                />
              )}
            </Reveal>
          ))}
        </div>

        <Reveal className="mt-12">
          <div className="surface-1 flex flex-col gap-3 rounded-lg border border-subtle p-5 md:flex-row md:items-center md:justify-between">
            <p className="text-sm text-mist">
              模式卡「开头避免天气描写」的置信度
            </p>
            <div className="flex items-center gap-3">
              <div
                className="h-2 w-48 rounded-full bg-white/10"
                role="presentation"
              >
                <div
                  className="h-2 rounded-full bg-moss"
                  style={{ width: "85%" }}
                />
              </div>
              <span className="text-sm text-moss-soft tabular-nums">85%</span>
            </div>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
