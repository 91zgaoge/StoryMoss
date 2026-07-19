import { PenLine, Package, Ruler } from "lucide-react";
import { Reveal, SectionHeader } from "./Reveal";

const ROLES = [
  {
    icon: PenLine,
    tag: "执笔",
    name: "主创 LeadWriter",
    desc: "读资产、写正文、按审查意见修订。创作力最强的模型为它服务。",
    offset: "md:mt-0",
  },
  {
    icon: Package,
    tag: "备料",
    name: "管理 Producer",
    desc: "生产世界观、角色卡、大纲与伏笔，调度模型与预算，把料备在写手前面。",
    offset: "md:mt-12",
  },
  {
    icon: Ruler,
    tag: "把关",
    name: "编辑审计 EditorAuditor",
    desc: "五维审查逐章终审：连续性、风格、合同兑现、AI 腔、追读力，引证据下裁决。",
    offset: "md:mt-24",
  },
];

export function TrioSection() {
  return (
    <section id="trio" className="relative px-6 py-24 md:py-36">
      <div className="mx-auto max-w-[1080px]">
        <SectionHeader
          kicker="三个创作者"
          title="一间工作室，三种手艺"
          lead="不是一次生成碰运气，而是三个代理围着同一块黑板协作：一个写、一个备料、一个终审，并行推进，稳定产出。"
        />

        <div className="grid gap-5 md:grid-cols-3">
          {ROLES.map((role) => (
            <Reveal key={role.name} className={role.offset}>
              <article className="surface-1 rounded-lg border border-subtle p-6">
                <div className="surface-2 mb-5 inline-flex h-10 w-10 items-center justify-center rounded-md text-moss">
                  <role.icon size={20} aria-hidden="true" />
                </div>
                <p className="mb-1 text-xs tracking-[0.2em] text-moss">
                  {role.tag}
                </p>
                <h3 className="mb-3 text-xl text-paper">{role.name}</h3>
                <p className="text-pretty text-sm leading-relaxed text-mist">
                  {role.desc}
                </p>
              </article>
            </Reveal>
          ))}
        </div>

        <Reveal className="mt-10">
          <p className="text-sm text-dim">
            黑板协作 · 并行推进 · 未过质量门的草稿不装配——稳定，才谈得上风格。
          </p>
        </Reveal>
      </div>
    </section>
  );
}
