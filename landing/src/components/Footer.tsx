export function Footer() {
  return (
    <footer className="border-t border-ink-line bg-parchment py-8">
      <div className="mx-auto flex max-w-[980px] flex-col items-center justify-between gap-4 px-6 md:flex-row">
        <p className="text-sm text-charcoal">© 2026 StoryForge · 草苔</p>
        <div className="flex gap-6 text-sm text-charcoal">
          <a
            href="https://github.com/91zgaoge/StoryForge"
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
