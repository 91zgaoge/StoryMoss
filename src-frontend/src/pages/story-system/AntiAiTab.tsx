import { useState } from 'react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { ShieldAlert, Zap, Loader2 } from 'lucide-react';
import { antiAiReview, evolveStyleFromAntiAiReview, logFeatureUsage } from '@/services/tauri';
import type { AntiAiReview } from '@/services/tauri';
import toast from 'react-hot-toast';

interface AntiAiTabProps {
  storyId: string;
  genre?: string | null;
}

export function AntiAiTab({ storyId, genre }: AntiAiTabProps) {
  const [reviewText, setReviewText] = useState('');
  const [reviewResult, setReviewResult] = useState<AntiAiReview | null>(null);
  const [isReviewing, setIsReviewing] = useState(false);
  const [isEvolving, setIsEvolving] = useState(false);

  const handleAntiAiReview = async () => {
    if (!reviewText.trim()) {
      toast.error('请输入要审查的文本');
      return;
    }
    setIsReviewing(true);
    try {
      const result = await antiAiReview(reviewText, genre || undefined);
      setReviewResult(result);
      toast.success('审查完成');
      logFeatureUsage('anti_ai_review', 'executed', storyId);
    } catch {
      toast.error('审查失败');
    } finally {
      setIsReviewing(false);
    }
  };

  const handleEvolveStyle = async () => {
    if (!reviewResult) return;
    setIsEvolving(true);
    try {
      const delta = await evolveStyleFromAntiAiReview(storyId, reviewResult);
      if (delta.reasons.length > 0) {
        toast.success(`风格已进化: ${delta.reasons.length} 项调整`);
      } else {
        toast.success('风格无需调整');
      }
    } catch {
      toast.error('风格进化失败');
    } finally {
      setIsEvolving(false);
    }
  };

  return (
    <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <Card>
        <CardContent className="p-4">
          <h3 className="text-lg font-semibold text-white mb-4 flex items-center gap-2">
            <ShieldAlert className="w-5 h-5 text-red-400" />
            Anti-AI 审查
          </h3>
          <textarea
            value={reviewText}
            onChange={e => setReviewText(e.target.value)}
            placeholder="粘贴要审查的文本..."
            className="w-full h-48 bg-cinema-800 border border-cinema-700 rounded-lg p-3 text-white text-sm resize-none focus:outline-none focus:border-cinema-gold"
          />
          <div className="flex justify-end mt-3">
            <Button
              onClick={handleAntiAiReview}
              disabled={isReviewing}
              className="flex items-center gap-2"
            >
              {isReviewing ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Zap className="w-4 h-4" />
              )}
              开始审查
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="p-4">
          <h3 className="text-lg font-semibold text-white mb-4">审查结果</h3>
          {!reviewResult ? (
            <p className="text-gray-500 text-sm">输入文本并点击审查</p>
          ) : (
            <div className="space-y-4">
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

              <div className="space-y-2">
                {reviewResult.dimensions.map(dim => (
                  <div key={dim.name} className="flex items-center gap-3">
                    <span className="text-gray-400 text-sm w-12">{dim.name}</span>
                    <div className="flex-1 h-4 bg-cinema-800 rounded-full overflow-hidden">
                      <div
                        className="h-full rounded-full"
                        style={{
                          width: `${dim.score * 100}%`,
                          backgroundColor:
                            dim.score > 0.7 ? '#4ade80' : dim.score > 0.4 ? '#fbbf24' : '#f87171',
                        }}
                      />
                    </div>
                    <span className="text-white text-xs w-8">{(dim.score * 100).toFixed(0)}</span>
                  </div>
                ))}
              </div>

              {reviewResult.issues.length > 0 && (
                <div className="mt-4">
                  <h4 className="text-white text-sm font-medium mb-2">发现的问题</h4>
                  <div className="space-y-2">
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
                        <p className="text-gray-400">{issue.description}</p>
                        <p className="text-cinema-gold text-xs mt-1">建议: {issue.suggestion}</p>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              <div className="mt-4">
                <Button size="sm" onClick={handleEvolveStyle} disabled={isEvolving}>
                  {isEvolving ? (
                    <Loader2 className="w-4 h-4 animate-spin mr-1" />
                  ) : (
                    <Zap className="w-4 h-4 mr-1" />
                  )}
                  接受审校并进化风格
                </Button>
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
