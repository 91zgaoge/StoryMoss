import { describe, it, expect, vi, beforeEach } from 'vitest';

const saveMock = vi.fn();
const writeFileMock = vi.fn();
const readFileMock = vi.fn();

vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: (...args: unknown[]) => saveMock(...args),
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
  writeFile: (...args: unknown[]) => writeFileMock(...args),
  readFile: (...args: unknown[]) => readFileMock(...args),
}));

import { saveExportViaDialog, type ExportResult } from './useExport';

describe('saveExportViaDialog', () => {
  beforeEach(() => {
    saveMock.mockReset();
    writeFileMock.mockReset();
    readFileMock.mockReset();
  });

  it('returns null when user cancels save dialog', async () => {
    saveMock.mockResolvedValue(null);
    const result: ExportResult = {
      file_path: '/tmp/exports/story.md',
      content: '# Hello',
    };
    await expect(saveExportViaDialog(result, 'markdown')).resolves.toBeNull();
    expect(writeFileMock).not.toHaveBeenCalled();
  });

  it('writes UTF-8 text for markdown/txt', async () => {
    saveMock.mockResolvedValue('/Users/me/小说.md');
    const result: ExportResult = {
      file_path: '/tmp/exports/story_20260709.md',
      content: '# 标题\n\n正文',
    };
    const path = await saveExportViaDialog(result, 'markdown');
    expect(path).toBe('/Users/me/小说.md');
    expect(writeFileMock).toHaveBeenCalledOnce();
    const [, bytes] = writeFileMock.mock.calls[0];
    expect(new TextDecoder().decode(bytes)).toBe('# 标题\n\n正文');
    expect(readFileMock).not.toHaveBeenCalled();
  });

  it('copies binary bytes for pdf/epub from backend temp path', async () => {
    saveMock.mockResolvedValue('/Users/me/书.epub');
    const bytes = new Uint8Array([0x50, 0x4b, 0x03, 0x04]);
    readFileMock.mockResolvedValue(bytes);
    const result: ExportResult = {
      file_path: '/tmp/exports/book.epub',
      content: '',
    };
    const path = await saveExportViaDialog(result, 'epub');
    expect(path).toBe('/Users/me/书.epub');
    expect(readFileMock).toHaveBeenCalledWith('/tmp/exports/book.epub');
    expect(writeFileMock).toHaveBeenCalledWith('/Users/me/书.epub', bytes);
  });

  it('rejects empty text content', async () => {
    saveMock.mockResolvedValue('/Users/me/empty.txt');
    const result: ExportResult = { file_path: '/tmp/empty.txt', content: '' };
    await expect(saveExportViaDialog(result, 'txt')).rejects.toThrow('导出内容为空');
  });
});
