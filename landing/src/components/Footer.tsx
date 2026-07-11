export function Footer() {
  return (
    <footer className="border-t border-ink-line bg-parchment py-10">
      <div className="mx-auto flex max-w-[980px] flex-col items-center justify-between gap-6 px-6 md:flex-row">
        <div className="flex items-center gap-2.5">
          <img
            src="/logo.png"
            alt="StoryMoss 草苔"
            className="h-7 w-7 object-contain"
          />
          <span className="font-display text-lg text-ink">草苔</span>
          <span className="font-body text-xs tracking-wide text-charcoal">StoryMoss</span>
        </div>

        <p className="text-sm text-charcoal">© 2026 StoryMoss · 草苔</p>

        <div className="flex gap-6 text-sm text-charcoal">
          <a
            href="https://github.com/91zgaoge/StoryMoss"
            className="hover:text-ink"
            target="_blank"
            rel="noreferrer"
          >
            GitHub
          </a>
          <a href="#" className="hover:text-ink">
            用户指南
          </a>
        </div>
      </div>
    </footer>
  );
}
