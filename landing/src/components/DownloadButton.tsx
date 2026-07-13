import type { ReactNode, AnchorHTMLAttributes } from "react";
import { useEffect, useState } from "react";

const RELEASE_BASE = "https://storymoss.top/releases";

const ASSETS = {
  mac: `${RELEASE_BASE}/StoryMoss_0.26.59_aarch64.dmg`,
  windows: `${RELEASE_BASE}/StoryMoss_0.26.59_x64_zh-CN.msi`,
  linux: `${RELEASE_BASE}/StoryMoss_0.26.59_amd64.AppImage`,
};

export type Platform = "mac" | "macIntel" | "windows" | "linux" | "unknown";

export function detectPlatform(): Platform {
  if (typeof navigator === "undefined") return "unknown";

  const ua = navigator.userAgent.toLowerCase();
  const platform = navigator.platform?.toLowerCase() || "";

  if (ua.includes("win")) return "windows";
  if (ua.includes("linux")) return "linux";
  if (platform.startsWith("mac") || ua.includes("mac")) {
    // Modern Apple Silicon Macs report MacIntel in user agent due to Rosetta,
    // but navigator.platform is also MacIntel. Default to Apple Silicon for
    // current Mac users; Intel users can pick the x64 build from releases.
    return "mac";
  }

  return "unknown";
}

export function downloadUrl(platform: Platform): string {
  if (platform === "windows") return ASSETS.windows;
  if (platform === "linux") return ASSETS.linux;
  if (platform === "mac") return ASSETS.mac;
  // macIntel and unknown fall back to the releases page so users can pick a build.
  return "https://storymoss.top/releases/";
}

export function downloadLabel(
  platform: Platform,
  fallback = "免费下载",
): string {
  if (platform === "mac" || platform === "macIntel") return "下载 macOS 版";
  if (platform === "windows") return "下载 Windows 版";
  if (platform === "linux") return "下载 Linux 版";
  return fallback;
}

type DownloadButtonProps = AnchorHTMLAttributes<HTMLAnchorElement> & {
  variant: "primary" | "secondary";
  children?: ReactNode;
  fallbackLabel?: string;
};

export function DownloadButton({
  variant,
  children,
  fallbackLabel,
  className = "",
  ...rest
}: DownloadButtonProps) {
  const [platform, setPlatform] = useState<Platform>("unknown");

  useEffect(() => {
    setPlatform(detectPlatform());
  }, []);

  const base =
    "inline-flex items-center justify-center rounded-[2px] px-6 py-3 text-sm font-medium transition-colors duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cinnabar focus-visible:ring-offset-2 focus-visible:ring-offset-parchment";
  const styles =
    variant === "primary"
      ? "bg-cinnabar text-white hover:bg-cinnabar-dark"
      : "border border-ink-line bg-parchment text-ink hover:border-cinnabar hover:text-cinnabar";

  const url = downloadUrl(platform);
  const label = children ?? downloadLabel(platform, fallbackLabel);

  return (
    <a
      href={url}
      target="_blank"
      rel="noreferrer"
      className={`${base} ${styles} ${className}`}
      {...rest}
    >
      {label}
    </a>
  );
}
