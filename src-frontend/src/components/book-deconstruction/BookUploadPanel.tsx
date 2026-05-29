import { useCallback } from 'react';
import { Upload, FileText, BookOpen } from 'lucide-react';
import toast from 'react-hot-toast';

interface BookUploadPanelProps {
  onUpload: (filePath: string) => void;
  isUploading: boolean;
}

export function BookUploadPanel({ onUpload, isUploading }: BookUploadPanelProps) {
  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      const file = e.dataTransfer.files[0];
      if (file) handleFile(file);
    },
    [onUpload]
  );

  const handleFileSelect = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (file) handleFile(file);
    },
    [onUpload]
  );

  const handleFile = (file: File) => {
    const ext = file.name.split('.').pop()?.toLowerCase();
    if (!['txt', 'pdf', 'epub'].includes(ext || '')) {
      toast.error('仅支持 txt、pdf、epub 格式');
      return;
    }
    if (file.size > 100 * 1024 * 1024) {
      toast.error('文件大小超过 100MB 限制');
      return;
    }
    // 获取文件路径（Tauri 环境下）
    if ('path' in file) {
      onUpload((file as any).path as string);
    } else {
      // 前端环境下，使用临时路径（实际需通过 Tauri dialog）
      toast.error('请使用文件选择器上传');
    }
  };

  const handleClick = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        filters: [{ name: '小说文件', extensions: ['txt', 'pdf', 'epub'] }],
      });
      if (selected && typeof selected === 'string') {
        onUpload(selected);
      }
    } catch {
      toast.error('无法打开文件选择器');
    }
  };

  return (
    <div
      onDrop={handleDrop}
      onDragOver={e => e.preventDefault()}
      className="flex flex-col items-center justify-center p-12 border-2 border-dashed border-cinema-700 rounded-2xl bg-cinema-900/50 hover:border-cinema-gold/50 hover:bg-cinema-900 transition-all cursor-pointer"
      onClick={handleClick}
    >
      <div className="w-16 h-16 rounded-full bg-cinema-800 flex items-center justify-center mb-4">
        <Upload className="w-8 h-8 text-cinema-gold" />
      </div>
      <h3 className="text-lg font-medium text-white mb-2">
        {isUploading ? '正在上传...' : '上传小说文件'}
      </h3>
      <p className="text-sm text-gray-500 text-center mb-4">支持 txt、pdf、epub 格式，最大 100MB</p>
      <div className="flex items-center gap-4 text-xs text-gray-600">
        <span className="flex items-center gap-1">
          <FileText className="w-3 h-3" /> TXT
        </span>
        <span className="flex items-center gap-1">
          <BookOpen className="w-3 h-3" /> PDF
        </span>
        <span className="flex items-center gap-1">
          <BookOpen className="w-3 h-3" /> EPUB
        </span>
      </div>
    </div>
  );
}
