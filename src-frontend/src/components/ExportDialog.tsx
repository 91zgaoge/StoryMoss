import { useState, useMemo } from 'react';
import { Download, FileText, BookOpen, Code, FileCode, FileType, LayoutTemplate } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useExport, useExportTemplates, type ExportFormat } from '@/hooks/useExport';

interface ExportDialogProps {
  storyId: string;
  storyTitle: string;
  isOpen: boolean;
  onClose: () => void;
}

const exportFormats: { id: ExportFormat; label: string; icon: typeof FileText; description: string }[] = [
  { id: 'markdown', label: 'Markdown', icon: FileText, description: 'Markdown格式，适合后续编辑' },
  { id: 'pdf', label: 'PDF', icon: BookOpen, description: 'PDF文档，适合分享和打印' },
  { id: 'epub', label: 'EPUB', icon: BookOpen, description: '电子书格式，适合阅读器' },
  { id: 'html', label: 'HTML', icon: Code, description: '网页格式，适合在线阅读' },
  { id: 'txt', label: '纯文本', icon: FileType, description: '纯文本格式，最通用' },
  { id: 'json', label: 'JSON', icon: FileCode, description: '数据格式，适合备份' },
];

export function ExportDialog({ storyId, storyTitle, isOpen, onClose }: ExportDialogProps) {
  const [selectedFormat, setSelectedFormat] = useState<ExportFormat>('markdown');
  const [includeMetadata, setIncludeMetadata] = useState(true);
  const [includeOutline, setIncludeOutline] = useState(true);
  const [includeCharacters, setIncludeCharacters] = useState(true);
  const [selectedTemplateId, setSelectedTemplateId] = useState<string | undefined>(undefined);

  const exportMutation = useExport();
  const { data: templates } = useExportTemplates(selectedFormat);

  const compatibleTemplates = useMemo(() => {
    if (!templates) return [];
    return templates.filter(t => t.format === selectedFormat || t.format === 'md' && selectedFormat === 'markdown' || t.format === 'txt' && selectedFormat === 'txt' || t.format === 'html' && selectedFormat === 'html');
  }, [templates, selectedFormat]);

  const handleExport = () => {
    exportMutation.mutate({
      story_id: storyId,
      format: selectedFormat,
      include_metadata: includeMetadata,
      include_outline: includeOutline,
      include_characters: includeCharacters,
      template_id: selectedTemplateId,
    }, {
      onSuccess: () => {
        onClose();
      },
    });
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 animate-fade-in">
      <Card className="w-full max-w-lg mx-4 animate-slide-up">
        <CardContent className="p-6">
          <div className="flex items-center gap-3 mb-6">
            <div className="p-2 rounded-xl bg-cinema-gold/10">
              <Download className="w-5 h-5 text-cinema-gold" />
            </div>
            <div>
              <h2 className="font-display text-xl font-bold text-white">导出故事</h2>
              <p className="text-sm text-gray-400">{storyTitle}</p>
            </div>
          </div>

          {/* Format Selection */}
          <div className="space-y-3 mb-6">
            <label className="text-sm text-gray-400">选择格式</label>
            <div className="grid grid-cols-2 gap-2">
              {exportFormats.map((format) => {
                const Icon = format.icon;
                return (
                  <button
                    key={format.id}
                    onClick={() => setSelectedFormat(format.id)}
                    className={`flex items-center gap-3 p-3 rounded-xl border transition-all text-left ${
                      selectedFormat === format.id
                        ? 'bg-cinema-gold/10 border-cinema-gold/50'
                        : 'bg-cinema-800/50 border-cinema-700 hover:border-cinema-600'
                    }`}
                  >
                    <Icon className={`w-5 h-5 ${
                      selectedFormat === format.id ? 'text-cinema-gold' : 'text-gray-500'
                    }`} />
                    <div className="flex-1 min-w-0">
                      <p className={`font-medium ${
                        selectedFormat === format.id ? 'text-white' : 'text-gray-300'
                      }`}>
                        {format.label}
                      </p>
                      <p className="text-xs text-gray-500 truncate">{format.description}</p>
                    </div>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Template Selection */}
          {compatibleTemplates.length > 0 && (
            <div className="space-y-3 mb-6">
              <label className="text-sm text-gray-400 flex items-center gap-2">
                <LayoutTemplate className="w-4 h-4" />
                选择模板
              </label>
              <div className="space-y-2">
                <button
                  onClick={() => setSelectedTemplateId(undefined)}
                  className={`w-full flex items-center gap-3 p-3 rounded-xl border transition-all text-left ${
                    !selectedTemplateId
                      ? 'bg-cinema-gold/10 border-cinema-gold/50'
                      : 'bg-cinema-800/50 border-cinema-700 hover:border-cinema-600'
                  }`}
                >
                  <div className="flex-1 min-w-0">
                    <p className={`font-medium ${!selectedTemplateId ? 'text-white' : 'text-gray-300'}`}>
                      默认样式
                    </p>
                    <p className="text-xs text-gray-500">使用内置默认排版</p>
                  </div>
                </button>
                {compatibleTemplates.map((template) => (
                  <button
                    key={template.id}
                    onClick={() => setSelectedTemplateId(template.id)}
                    className={`w-full flex items-center gap-3 p-3 rounded-xl border transition-all text-left ${
                      selectedTemplateId === template.id
                        ? 'bg-cinema-gold/10 border-cinema-gold/50'
                        : 'bg-cinema-800/50 border-cinema-700 hover:border-cinema-600'
                    }`}
                  >
                    <div className="flex-1 min-w-0">
                      <p className={`font-medium ${selectedTemplateId === template.id ? 'text-white' : 'text-gray-300'}`}>
                        {template.name}
                        {template.is_builtin && (
                          <span className="ml-2 text-xs px-1.5 py-0.5 rounded bg-cinema-700 text-gray-400">内置</span>
                        )}
                      </p>
                      {template.description && (
                        <p className="text-xs text-gray-500 truncate">{template.description}</p>
                      )}
                    </div>
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Options */}
          <div className="space-y-3 mb-6">
            <label className="text-sm text-gray-400">导出选项</label>
            <div className="space-y-2">
              <label className="flex items-center gap-3 p-3 rounded-xl bg-cinema-800/50 cursor-pointer hover:bg-cinema-800 transition-colors">
                <input
                  type="checkbox"
                  checked={includeMetadata}
                  onChange={(e) => setIncludeMetadata(e.target.checked)}
                  className="w-4 h-4 rounded border-cinema-600 bg-cinema-700 text-cinema-gold focus:ring-cinema-gold"
                />
                <span className="text-sm text-gray-300">包含元数据（标题、类型等）</span>
              </label>

              <label className="flex items-center gap-3 p-3 rounded-xl bg-cinema-800/50 cursor-pointer hover:bg-cinema-800 transition-colors">
                <input
                  type="checkbox"
                  checked={includeOutline}
                  onChange={(e) => setIncludeOutline(e.target.checked)}
                  className="w-4 h-4 rounded border-cinema-600 bg-cinema-700 text-cinema-gold focus:ring-cinema-gold"
                />
                <span className="text-sm text-gray-300">包含章节大纲</span>
              </label>

              <label className="flex items-center gap-3 p-3 rounded-xl bg-cinema-800/50 cursor-pointer hover:bg-cinema-800 transition-colors">
                <input
                  type="checkbox"
                  checked={includeCharacters}
                  onChange={(e) => setIncludeCharacters(e.target.checked)}
                  className="w-4 h-4 rounded border-cinema-600 bg-cinema-700 text-cinema-gold focus:ring-cinema-gold"
                />
                <span className="text-sm text-gray-300">包含角色介绍</span>
              </label>
            </div>
          </div>

          {/* Actions */}
          <div className="flex gap-3 pt-4 border-t border-cinema-700">
            <Button type="button" variant="ghost" onClick={onClose}>
              取消
            </Button>
            <Button
              variant="primary"
              onClick={handleExport}
              isLoading={exportMutation.isPending}
              className="flex-1 gap-2"
            >
              <Download className="w-4 h-4" />
              导出
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
