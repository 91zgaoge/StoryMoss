import { BookMarked, Map, Search, ShieldCheck } from "lucide-react";
import { Reveal, SectionHeader } from "./Reveal";

const PILLARS = [
  {
    icon: Search,
    tag: "记性",
    name: "写第 300 章，它去翻第 3 章",
    desc: "每次续写前，系统以本拍任务卡为线索（在场角色、地点、对峙双方、下一节点、你的指令）检索前文，把真正相关的原文摘录带进这次写作——不是只靠摘要和临时记忆。只读注入，可关。",
  },
  {
    icon: Map,
    tag: "计划",
    name: "卷与弧：长篇的中观路线",
    desc: "为每一卷规划目标、卷末高潮（必须不可逆）、须兑现的伏笔与线索预算。章纲按卷目标展开，写到卷末会提示规划下一卷——计划永远由你点按钮，系统不自动改写。",
  },
  {
    icon: BookMarked,
    tag: "改稿",
    name: "改了旧章，下游不会悄悄过期",
    desc: "编辑旧章后，受影响的后续摘要与快照自动重算；下游章纲标记「待重规划」，新旧对照逐条勾选才落库——你手写的章纲天然受保护。",
  },
  {
    icon: ShieldCheck,
    tag: "守卫",
    name: "死人不复活，秘密不提前说",
    desc: "写死的角色不会再作为活人行动；不在场者持有的物品不会被使用；角色说不出自己还不知道的秘密。每次提交自动跑一组四拍串联回归，回归即红。",
  },
];

export function LongformSection() {
  return (
    <section id="longform" className="relative px-6 py-24 md:py-36">
      <div className="mx-auto max-w-[1080px]">
        <SectionHeader
          kicker="长篇"
          title="写三百章，还认得前文"
          lead="连载真正的难关是「不崩」：前文记不住、计划漂移、死人复活、改一章乱十章。草苔把状态、计划、检索与守卫四件事都放在本地库里，逐拍、逐章地替你盯着。"
        />

        <div className="grid gap-5 md:grid-cols-2">
          {PILLARS.map((pillar) => (
            <Reveal key={pillar.name}>
              <article className="surface-1 h-full rounded-lg border border-subtle p-6">
                <div className="mb-5 flex items-center gap-3">
                  <div className="surface-2 inline-flex h-10 w-10 items-center justify-center rounded-md text-moss">
                    <pillar.icon size={20} aria-hidden="true" />
                  </div>
                  <p className="text-xs tracking-[0.2em] text-moss">
                    {pillar.tag}
                  </p>
                </div>
                <h3 className="mb-3 text-xl text-paper">{pillar.name}</h3>
                <p className="text-pretty text-sm leading-relaxed text-mist">
                  {pillar.desc}
                </p>
              </article>
            </Reveal>
          ))}
        </div>

        <Reveal className="mt-10">
          <p className="text-sm text-dim">
            状态跟踪 · 按需检索 · 卷纲 · 纲层重规划 ·
            跨拍守卫——全部本地运行，判定可引用原文。
          </p>
        </Reveal>
      </div>
    </section>
  );
}
