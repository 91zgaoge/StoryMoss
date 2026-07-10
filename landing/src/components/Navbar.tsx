import { useState, useEffect } from 'react';
import { Menu, X } from 'lucide-react';
import { InkRippleButton } from './InkRippleButton';

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
    { href: '#download', label: '下载' },
  ];

  return (
    <header
      className={`fixed top-0 left-0 right-0 z-50 h-[72px] transition-shadow duration-200 ${
        scrolled ? 'shadow-nav' : ''
      } border-b border-[#ddd7cd] bg-parchment/95 backdrop-blur-sm`}
    >
      <nav
        className="mx-auto flex h-full max-w-[1100px] items-center justify-between px-6"
        aria-label="主导航"
      >
        <a href="/" className="flex items-center gap-2 font-display text-xl text-ink">
          <span className="text-terracotta">草苔</span>
          <span className="font-sans text-sm tracking-wide text-stone">StoryForge</span>
        </a>

        <div className="hidden items-center gap-8 md:flex">
          {links.map((l) => (
            <a
              key={l.href}
              href={l.href}
              className="font-sans text-sm text-charcoal transition-colors hover:text-ink"
            >
              {l.label}
            </a>
          ))}
          <InkRippleButton variant="primary" className="px-5 py-2 text-sm">
            免费下载
          </InkRippleButton>
        </div>

        <button
          className="flex h-11 w-11 items-center justify-center rounded-md text-ink md:hidden"
          aria-label={mobileOpen ? '关闭菜单' : '打开菜单'}
          aria-expanded={mobileOpen}
          onClick={() => setMobileOpen((s) => !s)}
        >
          {mobileOpen ? <X size={24} /> : <Menu size={24} />}
        </button>
      </nav>

      {mobileOpen && (
        <div className="absolute left-0 right-0 top-[72px] border-b border-[#ddd7cd] bg-parchment px-6 py-4 md:hidden">
          <ul className="flex flex-col gap-4">
            {links.map((l) => (
              <li key={l.href}>
                <a
                  href={l.href}
                  className="block font-sans text-charcoal hover:text-ink"
                  onClick={() => setMobileOpen(false)}
                >
                  {l.label}
                </a>
              </li>
            ))}
            <li>
              <InkRippleButton variant="primary" className="w-full">
                免费下载
              </InkRippleButton>
            </li>
          </ul>
        </div>
      )}
    </header>
  );
}
