export function Footer() {
  return (
    <footer className="border-t border-[#ddd7cd] bg-parchment py-10">
      <div className="mx-auto flex max-w-[1100px] flex-col items-center justify-between gap-4 px-6 md:flex-row">
        <p className="text-sm text-stone">
          © 2026 StoryForge Team. 保留所有权利。
        </p>
        <nav className="flex gap-6" aria-label="页脚链接">
          <a href="https://github.com/91zgaoge/StoryForge" className="text-sm text-charcoal hover:text-ink">
            GitHub
          </a>
          <a href="#" className="text-sm text-charcoal hover:text-ink">
            用户指南
          </a>
          <a href="#" className="text-sm text-charcoal hover:text-ink">
            更新日志
          </a>
        </nav>
      </div>
    </footer>
  );
}
