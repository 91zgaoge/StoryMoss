import { useState, useEffect } from 'react';
import { useAppStore } from '@/stores/appStore';
import { getWritingAnalytics } from '@/services/tauri';
import { Card, CardContent } from '@/components/ui/Card';
import { FileText, Layers, Flame, Target, TrendingUp, Calendar, Loader2 } from 'lucide-react';
import type { WritingAnalytics } from '@/types/v3';

export function WritingStats({ embedded = false }: { embedded?: boolean }) {
  const currentStory = useAppStore(s => s.currentStory);
  const [analytics, setAnalytics] = useState<WritingAnalytics | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const fetchAnalytics = async () => {
      setIsLoading(true);
      try {
        if (currentStory?.id) {
          const data = await getWritingAnalytics(currentStory.id).catch(() => null);
          setAnalytics(data);
        } else {
          setAnalytics(null);
        }
      } catch (e) {
        console.warn('[WritingStats] fetch failed:', e);
      } finally {
        setIsLoading(false);
      }
    };

    fetchAnalytics();
  }, [currentStory?.id]);

  if (isLoading) {
    return (
      <div className={`flex items-center justify-center ${embedded ? 'py-16' : 'p-8 h-full'}`}>
        <Loader2 className="w-8 h-8 text-cinema-gold animate-spin" />
      </div>
    );
  }

  const formatWords = (n: number) => {
    if (n >= 10_000) return `${(n / 10_000).toFixed(1)}万`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
    return String(n);
  };

  const productivityColor = (score: number) => {
    if (score >= 80) return 'text-green-400';
    if (score >= 50) return 'text-cinema-gold';
    return 'text-gray-400';
  };

  const productivityBarColor = (score: number) => {
    if (score >= 80) return 'bg-green-400';
    if (score >= 50) return 'bg-cinema-gold';
    return 'bg-gray-500';
  };

  return (
    <div className={embedded ? 'space-y-6' : 'p-8 space-y-6 animate-fade-in'}>
      {!embedded && (
        <div className="flex items-center justify-between">
          <div>
            <h1 className="font-display text-3xl font-bold text-white">写作统计</h1>
            <p className="text-gray-400">
              {currentStory ? `${currentStory.title} - ` : ''}创作进度与写作习惯分析
            </p>
          </div>
        </div>
      )}

      {!currentStory ? (
        <Card>
          <CardContent className="p-8 text-center text-gray-500">
            请先在故事库中选择一个故事以查看写作统计
          </CardContent>
        </Card>
      ) : !analytics ? (
        <Card>
          <CardContent className="p-8 text-center text-gray-500">暂无写作数据</CardContent>
        </Card>
      ) : (
        <>
          {/* Stats Cards */}
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            <Card>
              <CardContent className="p-5">
                <div className="flex items-center justify-between">
                  <div>
                    <p className="text-xs text-gray-500 uppercase tracking-wider">总字数</p>
                    <p className="text-2xl font-bold text-white mt-1">
                      {formatWords(analytics.total_words)}
                    </p>
                  </div>
                  <FileText className="w-8 h-8 text-cinema-gold/40" />
                </div>
                <p className="text-xs text-gray-600 mt-2">{analytics.total_scenes} 个场景</p>
              </CardContent>
            </Card>

            <Card>
              <CardContent className="p-5">
                <div className="flex items-center justify-between">
                  <div>
                    <p className="text-xs text-gray-500 uppercase tracking-wider">总场景数</p>
                    <p className="text-2xl font-bold text-white mt-1">{analytics.total_scenes}</p>
                  </div>
                  <Layers className="w-8 h-8 text-blue-400/40" />
                </div>
                <p className="text-xs text-gray-600 mt-2">
                  平均每场景{' '}
                  {analytics.total_scenes > 0
                    ? Math.round(analytics.total_words / analytics.total_scenes)
                    : 0}{' '}
                  字
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardContent className="p-5">
                <div className="flex items-center justify-between">
                  <div>
                    <p className="text-xs text-gray-500 uppercase tracking-wider">连续写作</p>
                    <p className="text-2xl font-bold text-white mt-1">
                      {analytics.writing_streak.current_streak} 天
                    </p>
                  </div>
                  <Flame className="w-8 h-8 text-orange-400/40" />
                </div>
                <p className="text-xs text-gray-600 mt-2">
                  最高纪录: {analytics.writing_streak.longest_streak} 天
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardContent className="p-5">
                <div className="flex items-center justify-between">
                  <div>
                    <p className="text-xs text-gray-500 uppercase tracking-wider">生产力分数</p>
                    <p
                      className={`text-2xl font-bold mt-1 ${productivityColor(analytics.productivity_score)}`}
                    >
                      {Math.round(analytics.productivity_score)}
                    </p>
                  </div>
                  <Target className="w-8 h-8 text-purple-400/40" />
                </div>
                <div className="w-full bg-cinema-800 rounded-full h-1.5 mt-3">
                  <div
                    className={`h-1.5 rounded-full transition-all ${productivityBarColor(analytics.productivity_score)}`}
                    style={{ width: `${analytics.productivity_score}%` }}
                  />
                </div>
              </CardContent>
            </Card>
          </div>

          {/* Detail Cards */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Card>
              <CardContent className="p-5">
                <div className="flex items-center gap-2 mb-3">
                  <TrendingUp className="w-4 h-4 text-gray-400" />
                  <h2 className="font-display text-lg font-semibold text-white">日均产出</h2>
                </div>
                <div className="flex items-baseline gap-2">
                  <span className="text-3xl font-bold text-white">
                    {formatWords(Math.round(analytics.avg_words_per_day))}
                  </span>
                  <span className="text-gray-500">字/天</span>
                </div>
                <p className="text-sm text-gray-500 mt-2">基于实际写作天数计算</p>
              </CardContent>
            </Card>

            <Card>
              <CardContent className="p-5">
                <div className="flex items-center gap-2 mb-3">
                  <Calendar className="w-4 h-4 text-gray-400" />
                  <h2 className="font-display text-lg font-semibold text-white">最近写作</h2>
                </div>
                <div className="flex items-baseline gap-2">
                  <span className="text-xl font-bold text-white">
                    {analytics.writing_streak.last_writing_date
                      ? new Date(analytics.writing_streak.last_writing_date).toLocaleDateString(
                          'zh-CN'
                        )
                      : '无记录'}
                  </span>
                </div>
                <p className="text-sm text-gray-500 mt-2">
                  {analytics.writing_streak.current_streak > 0
                    ? `已连续写作 ${analytics.writing_streak.current_streak} 天，保持火力！`
                    : '今天还没有写作，开始创作吧'}
                </p>
              </CardContent>
            </Card>
          </div>
        </>
      )}
    </div>
  );
}
