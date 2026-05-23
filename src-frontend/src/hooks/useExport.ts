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
}

// MIME types for different export formats
const MIME_TYPES: Record<ExportFormat, string> = {
  markdown: 'text/markdown;charset=utf-8',
  pdf: 'application/pdf',
  epub: 'application/epub+zip',
  html: 'text/html;charset=utf-8',
  txt: 'text/plain;charset=utf-8',
  json: 'application/json;charset=utf-8',
};

// File extensions for different export formats
const FILE_EXTENSIONS: Record<ExportFormat, string> = {
  markdown: 'md',
  pdf: 'pdf',
  epub: 'epub',
  html: 'html',
  txt: 'txt',
  json: 'json',
};

async function exportStory(options: ExportOptions): Promise<ExportResult> {
  return loggedInvoke<ExportResult>('export_story', { options });
}

export function useExport() {
  return useMutation({
    mutationFn: exportStory,
    onSuccess: (data, variables) => {
      const { format } = variables;

      // For binary formats (PDF, EPUB), we need special handling
      // For now, we show a success message with the file path
      if (format === 'pdf' || format === 'epub') {
        toast.success(`导出成功！文件保存在: ${data.file_path}`);
        return;
      }

      // For text-based formats, trigger browser download
      if (!data.content) {
        toast.error('导出内容为空');
        return;
      }

      const blob = new Blob([data.content], { type: MIME_TYPES[format] });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;

      // Get filename from path or generate default
      const filename = data.file_path.split('\\').pop()?.split('/').pop()
        || `export.${FILE_EXTENSIONS[format]}`;
      a.download = filename;

      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);

      toast.success(`导出成功: ${filename}`);
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
      return loggedInvoke<ExportTemplate[]>('list_export_templates', { format_filter: formatFilter });
    },
  });
}

