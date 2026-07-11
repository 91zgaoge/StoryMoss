import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { DownloadButton, detectPlatform, downloadUrl, downloadLabel } from '../DownloadButton';

describe('DownloadButton', () => {
  beforeEach(() => {
    vi.stubGlobal('navigator', { userAgent: 'MacIntel', platform: 'MacIntel' });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('renders platform-specific label', () => {
    render(<DownloadButton variant="primary" />);
    expect(screen.getByRole('link', { name: /下载 macOS 版/i })).toBeInTheDocument();
  });

  it('points to GitHub release asset', () => {
    render(<DownloadButton variant="primary" />);
    const link = screen.getByRole('link', { name: /下载 macOS 版/i }) as HTMLAnchorElement;
    expect(link.href).toContain('github.com/91zgaoge/StoryMoss/releases/download/v0.26.59');
    expect(link.href).toContain('StoryMoss_0.26.59');
    expect(link.href).toMatch(/\.dmg$/);
  });

  it('falls back to releases page on unknown platform', () => {
    vi.stubGlobal('navigator', { userAgent: '', platform: '' });
    render(<DownloadButton variant="primary" />);
    const link = screen.getByRole('link') as HTMLAnchorElement;
    expect(link.href).toBe('https://github.com/91zgaoge/StoryMoss/releases/latest');
  });
});

describe('download helpers', () => {
  it('detects windows', () => {
    vi.stubGlobal('navigator', { userAgent: 'Windows NT 10.0', platform: 'Win32' });
    expect(detectPlatform()).toBe('windows');
  });

  it('detects linux', () => {
    vi.stubGlobal('navigator', { userAgent: 'X11; Linux x86_64', platform: 'Linux x86_64' });
    expect(detectPlatform()).toBe('linux');
  });

  it('detects mac', () => {
    vi.stubGlobal('navigator', { userAgent: 'Macintosh', platform: 'MacIntel' });
    expect(detectPlatform()).toBe('mac');
  });

  it('returns fallback labels', () => {
    expect(downloadLabel('unknown', '立即下载')).toBe('立即下载');
    expect(downloadLabel('windows')).toBe('下载 Windows 版');
  });

  it('returns fallback url for unknown platform', () => {
    expect(downloadUrl('unknown')).toBe('https://github.com/91zgaoge/StoryMoss/releases/latest');
  });
});
