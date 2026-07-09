import { useMutation, useQuery } from '@tanstack/react-query';
import { loggedInvoke } from '@/services/tauri';
import toast from 'react-hot-toast';

export type ExportFormat = 'markdown' | 'pdf' | 'epub' | 'html' | 'txt' | 'json';

export interface ExportTemplate {
  id: string;
  name: string;
  description?: string;
  format: string;
  template_content: string;
  is_builtin: boolean;
  is_user_created: boolean;
}

export interface ExportOptions {
  story_id: string;
  format: ExportFormat;
  include_metadata?: boolean;
  include_outline?: boolean;
  include_characters?: boolean;
  template_id?: string;
}

export interface ExportResult {
  file_path: string;
  content?: string;
  format?: string;
}

const FILE_EXTENSIONS: Record<ExportFormat, string> = {
  markdown: 'md',
  pdf: 'pdf',
  epub: 'epub',
  html: 'html',
  txt: 'txt',
  json: 'json',
};

const FILTER_LABELS: Record<ExportFormat, string> = {
  markdown: 'Markdown',
  pdf: 'PDF',
  epub: 'EPUB',
  html: 'HTML',
  txt: '纯文本',
  json: 'JSON',
};

const BINARY_FORMATS: ExportFormat[] = ['pdf', 'epub'];

function defaultFileName(filePath: string, format: ExportFormat): string {
  return filePath.split('\\').pop()?.split('/').pop() || `export.${FILE_EXTENSIONS[format]}`;
}

async function exportStory(options: ExportOptions): Promise<ExportResult> {
  return loggedInvoke<ExportResult>('export_story', { options });
}

/** 原生保存对话框：文本写 UTF-8，二进制从后端临时文件复制。取消返回 null。 */
export async function saveExportViaDialog(
  result: ExportResult,
  format: ExportFormat
): Promise<string | null> {
  const { save } = await import('@tauri-apps/plugin-dialog');
  const { writeFile, readFile } = await import('@tauri-apps/plugin-fs');

  const ext = FILE_EXTENSIONS[format];
  const suggested = defaultFileName(result.file_path, format);
  const filePath = await save({
    filters: [{ name: FILTER_LABELS[format], extensions: [ext] }],
    defaultPath: suggested,
  });
  if (!filePath) return null;

  if (BINARY_FORMATS.includes(format)) {
    const bytes = await readFile(result.file_path);
    await writeFile(filePath, bytes);
  } else {
    const text = result.content ?? '';
    if (!text) {
      throw new Error('导出内容为空');
    }
    await writeFile(filePath, new TextEncoder().encode(text));
  }
  return filePath;
}

export function useExport() {
  return useMutation({
    mutationFn: async (options: ExportOptions) => {
      const result = await exportStory(options);
      const savedPath = await saveExportViaDialog(result, options.format);
      return { result, savedPath, format: options.format };
    },
    onSuccess: data => {
      if (!data.savedPath) {
        toast('已取消保存', { icon: 'ℹ️' });
        return;
      }
      const name = data.savedPath.split('\\').pop()?.split('/').pop() || data.savedPath;
      toast.success(`导出成功: ${name}`);
    },
    onError: (error: Error) => {
      toast.error('导出失败: ' + error.message);
    },
  });
}

export function useExportTemplates(formatFilter?: string) {
  return useQuery({
    queryKey: ['export-templates', formatFilter],
    queryFn: async () => {
      return loggedInvoke<ExportTemplate[]>('list_export_templates', {
        format_filter: formatFilter,
      });
    },
  });
}
