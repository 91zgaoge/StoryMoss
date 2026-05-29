import { useState, useMemo } from 'react';
import {
  Download,
  FileText,
  BookOpen,
  Code,
  FileCode,
  FileType,
  LayoutTemplate,
  ShieldAlert,
  CheckCircle,
  AlertTriangle,
  ArrowRight,
  Loader2,
} from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useExport, useExportTemplates, type ExportFormat } from '@/hooks/useExport';
import { antiAiReview, getStoryChapters, logFeatureUsage } from '@/services/tauri';
import type { AntiAiReview } from '@/services/tauri';

interface ExportDialogProps {
  storyId: string;
  storyTitle: string;
  isOpen: boolean;
  onClose: () => void;
}

const exportFormats: {
  id: ExportFormat;
  label: string;
  icon: typeof FileText;
  description: string;
}[] = [
  { id: 'markdown', label: 'Markdown', icon: FileText, description: 'Markdown格式，适合后续编辑' },
  { id: 'pdf', label: 'PDF', icon: BookOpen, description: 'PDF文档，适合分享和打印' },
  { id: 'epub', label: 'EPUB', icon: BookOpen, description: '电子书格式，适合阅读器' },
  { id: 'html', label: 'HTML', icon: Code, description: '网页格式，适合在线阅读' },
  { id: 'txt', label: '纯文本', icon: FileType, description: '纯文本格式，最通用' },
  { id: 'json', label: 'JSON', icon: FileCode, description: '数据格式，适合备份' },
];

type Step = 'format' | 'health-check' | 'reviewing' | 'review-results';

export function ExportDialog({ storyId, storyTitle, isOpen, onClose }: ExportDialogProps) {
  const [step, setStep] = useState<Step>('format');
  const [selectedFormat, setSelectedFormat] = useState<ExportFormat>('markdown');
  const [includeMetadata, setIncludeMetadata] = useState(true);
  const [includeOutline, setIncludeOutline] = useState(true);
  const [includeCharacters, setIncludeCharacters] = useState(true);
  const [selectedTemplateId, setSelectedTemplateId] = useState<string | undefined>(undefined);

  const [reviewResult, setReviewResult] = useState<AntiAiReview | null>(null);
  const [isReviewing, setIsReviewing] = useState(false);

  const exportMutation = useExport();
  const { data: templates } = useExportTemplates(selectedFormat);

  const compatibleTemplates = useMemo(() => {
    if (!templates) return [];
    return templates.filter(
      t =>
        t.format === selectedFormat ||
        (t.format === 'md' && selectedFormat === 'markdown') ||
        (t.format === 'txt' && selectedFormat === 'txt') ||
        (t.format === 'html' && selectedFormat === 'html')
    );
  }, [templates, selectedFormat]);

  const handleExport = () => {
    exportMutation.mutate(
      {
        story_id: storyId,
        format: selectedFormat,
        include_metadata: includeMetadata,
        include_outline: includeOutline,
        include_characters: includeCharacters,
        template_id: selectedTemplateId,
      },
      {
        onSuccess: () => {
          onClose();
        },
      }
    );
  };

  const handleRunAntiAi = async () => {
    setStep('reviewing');
    setIsReviewing(true);
    try {
      // Fetch all chapters and concatenate content
      const chapters = await getStoryChapters(storyId);
      const fullText = chapters
        .map(c => c.content || '')
        .filter(Boolean)
        .join('\n\n');
      if (!fullText.trim()) {
        setReviewResult(null);
        setStep('review-results');
        setIsReviewing(false);
        return;
      }
      // Limit to first 3000 chars to avoid overwhelming the review
      const textToReview = fullText.slice(0, 3000);
      const result = await antiAiReview(textToReview);
      setReviewResult(result);
      logFeatureUsage('anti_ai_review', 'executed', storyId);
    } catch (e) {
      // silent fail - allow export anyway
      setReviewResult(null);
    } finally {
      setIsReviewing(false);
      setStep('review-results');
    }
  };

  const handleSkipAntiAi = () => {
    handleExport();
  };

  const handleBackToFormat = () => {
    setStep('format');
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

          {step === 'format' && (
            <>
              {/* Format Selection */}
              <div className="space-y-3 mb-6">
                <label className="text-sm text-gray-400">选择格式</label>
                <div className="grid grid-cols-2 gap-2">
                  {exportFormats.map(format => {
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
                        <Icon
                          className={`w-5 h-5 ${
                            selectedFormat === format.id ? 'text-cinema-gold' : 'text-gray-500'
                          }`}
                        />
                        <div className="flex-1 min-w-0">
                          <p
                            className={`font-medium ${
                              selectedFormat === format.id ? 'text-white' : 'text-gray-300'
                            }`}
                          >
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
                        <p
                          className={`font-medium ${!selectedTemplateId ? 'text-white' : 'text-gray-300'}`}
                        >
                          默认样式
                        </p>
                        <p className="text-xs text-gray-500">使用内置默认排版</p>
                      </div>
                    </button>
                    {compatibleTemplates.map(template => (
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
                          <p
                            className={`font-medium ${selectedTemplateId === template.id ? 'text-white' : 'text-gray-300'}`}
                          >
                            {template.name}
                            {template.is_builtin && (
                              <span className="ml-2 text-xs px-1.5 py-0.5 rounded bg-cinema-700 text-gray-400">
                                内置
                              </span>
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
                      onChange={e => setIncludeMetadata(e.target.checked)}
                      className="w-4 h-4 rounded border-cinema-600 bg-cinema-700 text-cinema-gold focus:ring-cinema-gold"
                    />
                    <span className="text-sm text-gray-300">包含元数据（标题、类型等）</span>
                  </label>

                  <label className="flex items-center gap-3 p-3 rounded-xl bg-cinema-800/50 cursor-pointer hover:bg-cinema-800 transition-colors">
                    <input
                      type="checkbox"
                      checked={includeOutline}
                      onChange={e => setIncludeOutline(e.target.checked)}
                      className="w-4 h-4 rounded border-cinema-600 bg-cinema-700 text-cinema-gold focus:ring-cinema-gold"
                    />
                    <span className="text-sm text-gray-300">包含章节大纲</span>
                  </label>

                  <label className="flex items-center gap-3 p-3 rounded-xl bg-cinema-800/50 cursor-pointer hover:bg-cinema-800 transition-colors">
                    <input
                      type="checkbox"
                      checked={includeCharacters}
                      onChange={e => setIncludeCharacters(e.target.checked)}
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
                  onClick={() => {
                    setStep('health-check');
                    logFeatureUsage('anti_ai_review', 'opened', storyId);
                  }}
                  className="flex-1 gap-2"
                >
                  <ArrowRight className="w-4 h-4" />
                  下一步
                </Button>
              </div>
            </>
          )}

          {step === 'health-check' && (
            <>
              <div className="space-y-4 mb-6">
                <div className="p-4 rounded-xl bg-cinema-800/50 border border-cinema-700">
                  <div className="flex items-center gap-3 mb-3">
                    <div className="p-2 rounded-lg bg-cinema-gold/10">
                      <ShieldAlert className="w-5 h-5 text-cinema-gold" />
                    </div>
                    <div>
                      <h3 className="text-white font-medium">出版前体检</h3>
                      <p className="text-xs text-gray-400">
                        运行 Anti-AI 审查，检测文本中的 AI 痕迹
                      </p>
                    </div>
                  </div>

                  <div className="space-y-2">
                    <p className="text-xs text-gray-400">
                      导出前可选择运行 Anti-AI 审查，帮助识别文本中可能被读者察觉的 AI 生成痕迹。
                    </p>
                  </div>
                </div>
              </div>

              <div className="flex gap-3 pt-4 border-t border-cinema-700">
                <Button type="button" variant="ghost" onClick={handleBackToFormat}>
                  返回
                </Button>
                <Button variant="secondary" onClick={handleSkipAntiAi} className="flex-1">
                  跳过，直接导出
                </Button>
                <Button variant="primary" onClick={handleRunAntiAi} className="flex-1 gap-2">
                  <ShieldAlert className="w-4 h-4" />
                  运行审查
                </Button>
              </div>
            </>
          )}

          {step === 'reviewing' && (
            <div className="py-12 text-center space-y-4">
              <Loader2 className="w-8 h-8 text-cinema-gold animate-spin mx-auto" />
              <p className="text-white font-medium">正在运行 Anti-AI 审查...</p>
              <p className="text-xs text-gray-400">正在分析文本的 AI 生成特征，请稍候</p>
            </div>
          )}

          {step === 'review-results' && (
            <>
              <div className="space-y-4 mb-6 max-h-96 overflow-y-auto">
                {reviewResult ? (
                  <>
                    <div className="flex items-center gap-4">
                      <div
                        className="text-3xl font-bold"
                        style={{
                          color:
                            reviewResult.overall_score > 0.7
                              ? '#4ade80'
                              : reviewResult.overall_score > 0.4
                                ? '#fbbf24'
                                : '#f87171',
                        }}
                      >
                        {(reviewResult.overall_score * 100).toFixed(0)}
                      </div>
                      <div className="text-gray-400 text-sm">综合评分</div>
                    </div>

                    {reviewResult.issues.length > 0 ? (
                      <div className="space-y-2">
                        <p className="text-xs text-gray-400">
                          发现 {reviewResult.issues.length} 个问题：
                        </p>
                        {reviewResult.issues.slice(0, 5).map((issue, idx) => (
                          <div key={idx} className="p-2 bg-cinema-800 rounded text-sm">
                            <div className="flex items-center gap-2 mb-1">
                              <span
                                className={`text-xs px-1.5 py-0.5 rounded ${
                                  issue.severity === 'high'
                                    ? 'bg-red-900/50 text-red-300'
                                    : issue.severity === 'medium'
                                      ? 'bg-yellow-900/50 text-yellow-300'
                                      : 'bg-blue-900/50 text-blue-300'
                                }`}
                              >
                                {issue.severity}
                              </span>
                              <span className="text-gray-300">{issue.dimension}</span>
                            </div>
                            <p className="text-gray-400 text-xs">{issue.description}</p>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <div className="flex items-center gap-2 text-green-400">
                        <CheckCircle className="w-4 h-4" />
                        <span className="text-sm">未发现明显 AI 痕迹</span>
                      </div>
                    )}
                  </>
                ) : (
                  <div className="text-center py-4">
                    <AlertTriangle className="w-6 h-6 text-yellow-400 mx-auto mb-2" />
                    <p className="text-sm text-gray-300">审查未返回结果</p>
                    <p className="text-xs text-gray-500 mt-1">可能是文本为空或审查服务暂时不可用</p>
                  </div>
                )}
              </div>

              <div className="flex gap-3 pt-4 border-t border-cinema-700">
                <Button type="button" variant="ghost" onClick={() => setStep('health-check')}>
                  返回
                </Button>
                <Button
                  variant="secondary"
                  onClick={handleExport}
                  isLoading={exportMutation.isPending}
                  className="flex-1"
                >
                  忽略并导出
                </Button>
              </div>
            </>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
