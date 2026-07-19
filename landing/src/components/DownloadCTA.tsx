import { DownloadButton } from "./DownloadButton";
import { Reveal } from "./Reveal";

export function DownloadCTA() {
  return (
    <section
      id="download"
      className="relative overflow-hidden px-6 py-24 md:py-36"
    >
      <div
        className="pointer-events-none absolute -top-24 left-1/3 h-72 w-[560px] rounded-full bg-moss opacity-[0.06] blur-3xl"
        aria-hidden="true"
      />
      <div className="relative mx-auto max-w-[1080px]">
        <Reveal className="max-w-[640px]">
          <h2 className="text-balance mb-6 text-3xl leading-[1.15] tracking-mid text-paper md:text-[44px]">
            现在，把工作室
            <br />
            搬回你的书桌
          </h2>
          <p className="text-pretty mb-10 max-w-[520px] leading-relaxed text-mist">
            v0.30.0 ·
            多代理创作框架、持续学习、创作评估全部就位。本地运行，开源可审计。
          </p>
          <div className="flex flex-col items-start gap-4 sm:flex-row sm:items-center">
            <DownloadButton variant="primary" />
            <a
              href="https://github.com/91zgaoge/StoryMoss"
              target="_blank"
              rel="noreferrer"
              className="text-sm text-mist underline decoration-dim underline-offset-4 transition-colors hover:text-paper"
            >
              在 GitHub 查看源码
            </a>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
