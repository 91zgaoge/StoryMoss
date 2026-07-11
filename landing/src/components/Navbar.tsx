import { useState, useEffect } from 'react';
import { Menu, X } from 'lucide-react';
import { DownloadButton } from './DownloadButton';

export function Navbar() {
  const [scrolled, setScrolled] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 8);
    window.addEventListener('scroll', onScroll);
    return () => window.removeEventListener('scroll', onScroll);
  }, []);

  const links = [
    { href: '#features', label: '功能' },
    { href: '#approach', label: '方法' },
    { href: '#genesis', label: '创世' },
    { href: '#quickstart', label: '上手' },
    { href: '#download', label: '下载' },
  ];

  return (
    <header
      className={`fixed top-0 left-0 right-0 z-50 h-[72px] border-b border-ink-line bg-parchment/95 backdrop-blur-sm transition-shadow duration-200 ${
        scrolled ? 'shadow-nav' : ''
      }`}
    >
      <nav
        className="mx-auto flex h-full max-w-[980px] items-center justify-between px-6"
        aria-label="主导航"
      >
        <a href="/" className="flex items-center gap-2.5 font-display text-xl text-ink">
          <img
            src="/logo.png"
            alt="StoryMoss 草苔"
            className="h-8 w-8 object-contain"
          />
          <span>草苔</span>
          <span className="font-body text-xs tracking-wide text-charcoal">StoryMoss</span>
        </a>

        <div className="hidden items-center gap-8 md:flex">
          {links.map((l) => (
            <a
              key={l.href}
              href={l.href}
              className="text-sm text-charcoal transition-colors hover:text-ink"
            >
              {l.label}
            </a>
          ))}
          <DownloadButton
            variant="primary"
            className="px-5 py-2 text-xs"
            aria-label="免费下载"
          >
            免费下载
          </DownloadButton>
        </div>

        <button
          className="flex h-10 w-10 items-center justify-center rounded-[2px] text-ink md:hidden"
          aria-label={mobileOpen ? '关闭菜单' : '打开菜单'}
          aria-expanded={mobileOpen}
          onClick={() => setMobileOpen((s) => !s)}
        >
          {mobileOpen ? <X size={22} /> : <Menu size={22} />}
        </button>
      </nav>

      {mobileOpen && (
        <div className="absolute left-0 right-0 top-[72px] border-b border-ink-line bg-parchment px-6 py-5 md:hidden">
          <ul className="flex flex-col gap-4">
            {links.map((l) => (
              <li key={l.href}>
                <a
                  href={l.href}
                  className="block text-charcoal hover:text-ink"
                  onClick={() => setMobileOpen(false)}
                >
                  {l.label}
                </a>
              </li>
            ))}
            <li>
              <DownloadButton variant="primary" className="w-full">
                免费下载
              </DownloadButton>
            </li>
          </ul>
        </div>
      )}
    </header>
  );
}
