import { useState, useEffect } from "react";
import { Menu, X } from "lucide-react";
import { DownloadButton } from "./DownloadButton";

export function Navbar() {
  const [mobileOpen, setMobileOpen] = useState(false);

  useEffect(() => {
    document.body.style.overflow = mobileOpen ? "hidden" : "";
    return () => {
      document.body.style.overflow = "";
    };
  }, [mobileOpen]);

  const links = [
    { href: "#trio", label: "协作" },
    { href: "#learning", label: "学习" },
    { href: "#craft", label: "品质" },
    { href: "#screens", label: "界面" },
    { href: "#download", label: "下载" },
  ];

  return (
    <header className="fixed left-0 right-0 top-0 z-50 h-[72px] border-b border-subtle bg-canvas/90 backdrop-blur-sm">
      <nav
        className="mx-auto flex h-full max-w-[1080px] items-center justify-between px-6"
        aria-label="主导航"
      >
        <a
          href="/"
          className="flex items-center gap-2.5 font-display text-xl text-paper"
        >
          <img
            src="/logo.png"
            alt="StoryMoss 草苔"
            className="h-8 w-8 object-contain invert"
          />
          <span>草苔</span>
          <span className="font-body text-xs tracking-wide text-mist">
            StoryMoss
          </span>
        </a>

        <div className="hidden items-center gap-8 md:flex">
          {links.map((l) => (
            <a
              key={l.href}
              href={l.href}
              className="text-sm text-mist transition-colors hover:text-paper"
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
          className="flex h-10 w-10 items-center justify-center rounded-md text-paper md:hidden"
          aria-label={mobileOpen ? "关闭菜单" : "打开菜单"}
          aria-expanded={mobileOpen}
          onClick={() => setMobileOpen((s) => !s)}
        >
          {mobileOpen ? <X size={22} /> : <Menu size={22} />}
        </button>
      </nav>

      {mobileOpen && (
        <div className="absolute left-0 right-0 top-[72px] border-b border-subtle bg-canvas-2 px-6 py-5 md:hidden">
          <ul className="flex flex-col gap-4">
            {links.map((l) => (
              <li key={l.href}>
                <a
                  href={l.href}
                  className="block text-mist hover:text-paper"
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
