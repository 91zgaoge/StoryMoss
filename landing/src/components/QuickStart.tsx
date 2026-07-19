import { Reveal, SectionHeader } from "./Reveal";

const STEPS = [
  {
    no: "01",
    title: "装上你的模型",
    desc: "OpenAI、Anthropic、DeepSeek、Qwen 或本地 Ollama——填地址和 Key，测试连接即可。",
  },
  {
    no: "02",
    title: "一句话创世",
    desc: "幕前输入「写一部……」，三个代理即刻开工：管理备料、主创作首章、编辑审计把关。",
  },
  {
    no: "03",
    title: "看着它长",
    desc: "代理工作室里看三角色实时协作；写几章后，学习中心开始给你攒技能。",
  },
];

export function QuickStart() {
  return (
    <section id="quickstart" className="relative px-6 py-24 md:py-36">
      <div className="mx-auto max-w-[1080px]">
        <SectionHeader kicker="上手" title="三步，开一间工作室" />

        <div className="grid gap-5 md:grid-cols-3">
          {STEPS.map((step) => (
            <Reveal key={step.no}>
              <article className="surface-1 h-full rounded-lg border border-subtle p-6">
                <p
                  className="mb-4 font-display text-3xl text-moss"
                  aria-hidden="true"
                >
                  {step.no}
                </p>
                <h3 className="mb-3 text-xl text-paper">{step.title}</h3>
                <p className="text-pretty text-sm leading-relaxed text-mist">
                  {step.desc}
                </p>
              </article>
            </Reveal>
          ))}
        </div>
      </div>
    </section>
  );
}
