import { useEffect, useState } from "react";

/**
 * Latest release manifest served from the same origin as the landing page.
 * `latest.json` is the Tauri updater manifest; it is rewritten on every release
 * and is never removed by the FTP retention cleanup (it is an unversioned file).
 * Fetching it at runtime keeps the download page in sync with the newest release
 * without requiring a landing redeploy on each release.
 */
const LATEST_MANIFEST_URL = "https://storymoss.top/releases/latest.json";
const RELEASE_BASE = "https://storymoss.top/releases";

/**
 * Fallback version used when the manifest cannot be fetched (offline, server
 * down, etc.). MUST be bumped alongside the release version in Cargo.toml /
 * package.json so a failed fetch still points to a valid, retained version.
 */
// v0.64.2：v0.64.0 于 2026-10-06 完整发布到 storymoss.top/releases
// （latest.json=0.64.0，dmg/msi/AppImage/deb 全部 200）；兜底版本随发版同步
// bump，保留策略保留最近 5 个版本。
export const FALLBACK_VERSION = "0.64.5";

export type Platform = "mac" | "windows" | "linux";

export interface ReleaseUrls {
  mac: string;
  windows: string;
  linux: string;
}

export interface LatestRelease {
  version: string;
  urls: ReleaseUrls;
}

/**
 * Build download URLs from a version string using the confirmed bundle naming:
 *   macOS:   StoryMoss_{version}_aarch64.dmg
 *   Windows: StoryMoss_{version}_x64_zh-CN.msi
 *   Linux:   StoryMoss_{version}_amd64.AppImage
 *
 * `latest.json`'s macOS entry is the `.app.tar.gz` update artifact (unversioned),
 * not the `.dmg` download artifact, so the `.dmg` URL is always derived from the
 * version field.
 */
export function buildReleaseUrls(version: string): ReleaseUrls {
  return {
    mac: `${RELEASE_BASE}/StoryMoss_${version}_aarch64.dmg`,
    windows: `${RELEASE_BASE}/StoryMoss_${version}_x64_zh-CN.msi`,
    linux: `${RELEASE_BASE}/StoryMoss_${version}_amd64.AppImage`,
  };
}

export const FALLBACK_RELEASE: LatestRelease = {
  version: FALLBACK_VERSION,
  urls: buildReleaseUrls(FALLBACK_VERSION),
};

/**
 * Normalize a manifest version string: strip a leading "v" so "v0.30.27" and
 * "0.30.27" are treated identically.
 */
function normalizeVersion(raw: unknown): string {
  const v = String(raw ?? "").trim();
  return v.startsWith("v") || v.startsWith("V") ? v.slice(1) : v;
}

// Module-level cache + single in-flight promise: Hero / DownloadCTA /
// DownloadButton all share one network request for the manifest.
let cache: LatestRelease | null = null;
let pending: Promise<LatestRelease> | null = null;

async function fetchLatest(): Promise<LatestRelease> {
  if (cache) return cache;
  if (pending) return pending;

  pending = (async () => {
    try {
      const res = await fetch(LATEST_MANIFEST_URL, { cache: "no-store" });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as { version?: unknown };
      const version = normalizeVersion(data.version) || FALLBACK_VERSION;
      cache = { version, urls: buildReleaseUrls(version) };
      return cache;
    } catch {
      // Network/parse failure: fall back to a known-good retained version so the
      // download links never 404 just because the manifest fetch failed.
      cache = FALLBACK_RELEASE;
      return cache;
    } finally {
      pending = null;
    }
  })();

  return pending;
}

/**
 * Reset the module-level cache. Exported for tests so each case can start clean.
 */
export function __resetLatestReleaseCache(): void {
  cache = null;
  pending = null;
}

/**
 * React hook returning the latest release info. Renders immediately with the
 * fallback (or cached value) and updates once the manifest resolves.
 */
export function useLatestRelease(): LatestRelease {
  const [release, setRelease] = useState<LatestRelease>(
    () => cache ?? FALLBACK_RELEASE,
  );

  useEffect(() => {
    let mounted = true;
    fetchLatest().then((resolved) => {
      if (mounted) setRelease(resolved);
    });
    return () => {
      mounted = false;
    };
  }, []);

  return release;
}
