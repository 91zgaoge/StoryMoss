import { Reveal, SectionHeader } from "./Reveal";

const GRADERS = [
  {
    key: "code",
    name: "规则校验",
    desc: "字数、结构、自重复率——不合格不出闸。",
  },
  { key: "rule", name: "合同红线", desc: "人设、世界观、禁则区，一条不越。" },
  {
    key: "model",
    name: "编辑裁决",
    desc: "五维审查打分，每条意见都引用原文作证。",
  },
  {
    key: "human",
    name: "你的手感",
    desc: "你改了多少，它记多少——修改率回流成信号。",
  },
];

function ThresholdRuler() {
  return (
    <div className="surface-1 rounded-lg border border-subtle p-6">
      <p className="mb-4 text-sm text-mist">质量门加权分阈值</p>
      <div
        className="relative h-2 rounded-full bg-white/10"
        role="presentation"
      >
        <div className="h-2 rounded-full bg-moss" style={{ width: "75%" }} />
        <div
          className="absolute top-[-6px] h-4 w-px bg-moss-soft"
          style={{ left: "75%" }}
          aria-hidden="true"
        />
      </div>
      <div className="mt-2 flex justify-between text-xs text-dim">
        <span>0</span>
        <span className="text-moss-soft tabular-nums">0.75</span>
        <span>1.0</span>
      </div>
      <p className="text-pretty mt-4 text-sm leading-relaxed text-mist">
        低于阈值：草稿带审查意见回流修订，最多两轮；两轮不过，交还给你而不是硬发。
      </p>
    </div>
  );
}

export function CraftSection() {
  return (
    <section id="craft" className="relative px-6 py-24 md:py-36">
      <div className="mx-auto max-w-[1080px]">
        <SectionHeader
          kicker="品质"
          title="每一章，都要过审"
          lead="四级评分者从机械规则到你的手感层层把关；评分、检查点、用量全部留痕，打开「创作评估」就能看见工作室的健康曲线。"
        />

        <div className="grid gap-5 md:grid-cols-2">
          <div className="space-y-4">
            {GRADERS.map((g) => (
              <Reveal key={g.key}>
                <div className="flex items-baseline gap-4">
                  <span className="w-16 shrink-0 text-xs tracking-[0.2em] text-moss">
                    {g.key.toUpperCase()}
                  </span>
                  <div>
                    <h3 className="text-base text-paper">{g.name}</h3>
                    <p className="text-sm text-mist">{g.desc}</p>
                  </div>
                </div>
              </Reveal>
            ))}
          </div>
          <Reveal>
            <ThresholdRuler />
          </Reveal>
        </div>

        <Reveal className="mt-10">
          <p className="text-sm text-dim">
            里程碑检查点支持「现在 vs 当时」对比；评估场景随 CI 运行，回归即红。
          </p>
        </Reveal>
      </div>
    </section>
  );
}
