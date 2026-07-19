export function Footer() {
  return (
    <footer className="border-t border-subtle px-6 py-12">
      <div className="mx-auto flex max-w-[1080px] flex-col gap-6 md:flex-row md:items-center md:justify-between">
        <div className="flex items-center gap-2.5">
          <img
            src="/logo.png"
            alt=""
            className="h-6 w-6 object-contain invert"
          />
          <span className="font-display text-paper">草苔 StoryMoss</span>
          <span className="text-sm text-dim">越写越懂</span>
        </div>
        <nav
          className="flex flex-wrap gap-6 text-sm text-mist"
          aria-label="页脚导航"
        >
          <a
            href="https://github.com/91zgaoge/StoryMoss"
            target="_blank"
            rel="noreferrer"
            className="transition-colors hover:text-paper"
          >
            GitHub
          </a>
          <a
            href="https://storymoss.top/releases/"
            target="_blank"
            rel="noreferrer"
            className="transition-colors hover:text-paper"
          >
            全部版本
          </a>
          <a
            href="https://github.com/91zgaoge/StoryMoss/blob/master/CHANGELOG.md"
            target="_blank"
            rel="noreferrer"
            className="transition-colors hover:text-paper"
          >
            更新日志
          </a>
        </nav>
        <p className="text-xs text-dim">Made with 🌿 by StoryMoss Team</p>
      </div>
    </footer>
  );
}
