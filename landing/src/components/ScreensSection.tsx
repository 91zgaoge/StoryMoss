import { Reveal, SectionHeader } from "./Reveal";

const SHOTS = [
  {
    src: "/screenshots/frontstage.png",
    alt: "幕前写作界面：沉浸式编辑器",
    caption: "幕前写作 · 无干扰码字台",
    main: true,
  },
  {
    src: "/screenshots/dashboard.png",
    alt: "幕后故事库：当前编辑、创作入口与故事资产一览",
    caption: "幕后 · 故事库与创作入口",
  },
  {
    src: "/screenshots/knowledge-graph.png",
    alt: "知识图谱页的记忆健康：实体数、优先级与归档建议",
    caption: "知识图谱 · 实体与关系健康",
  },
];

export function ScreensSection() {
  const [main, ...rest] = SHOTS;
  return (
    <section id="screens" className="relative px-6 py-24 md:py-36">
      <div className="mx-auto max-w-[1080px]">
        <SectionHeader
          kicker="界面"
          title="一间书斋的样子"
          lead="幕后把故事、角色、世界观管得清清楚楚；幕前只剩你和正文。"
        />

        <Reveal className="mb-5">
          <figure className="surface-1 overflow-hidden rounded-lg border border-subtle p-2">
            <img
              src={main.src}
              alt={main.alt}
              width="1600"
              height="900"
              loading="lazy"
              className="w-full rounded-md outline outline-1 outline-offset-[-1px] outline-white/10"
            />
            <figcaption className="px-2 py-3 text-sm text-mist">
              {main.caption}
            </figcaption>
          </figure>
        </Reveal>

        <div className="grid gap-5 md:grid-cols-2">
          {rest.map((shot) => (
            <Reveal key={shot.src}>
              <figure className="surface-1 overflow-hidden rounded-lg border border-subtle p-2">
                <img
                  src={shot.src}
                  alt={shot.alt}
                  width="1200"
                  height="675"
                  loading="lazy"
                  className="w-full rounded-md outline outline-1 outline-offset-[-1px] outline-white/10"
                />
                <figcaption className="px-2 py-3 text-sm text-mist">
                  {shot.caption}
                </figcaption>
              </figure>
            </Reveal>
          ))}
        </div>
      </div>
    </section>
  );
}
