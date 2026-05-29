import { useState, useEffect } from 'react';
import { useAppStore } from '@/stores/appStore';
import { getLlmCallStats, getRecentLlmCalls, getStoryLlmCalls } from '@/services/tauri';
import { Card, CardContent } from '@/components/ui/Card';
import {
  BarChart3,
  Coins,
  Hash,
  Activity,
  Clock,
  CheckCircle,
  XCircle,
  Loader2,
} from 'lucide-react';
import type { LlmCall } from '@/types';

export function UsageStats() {
  const currentStory = useAppStore(s => s.currentStory);
  const [globalStats, setGlobalStats] = useState<{
    count: number;
    total_tokens: number;
    total_cost: number;
  } | null>(null);
  const [storyStats, setStoryStats] = useState<{
    count: number;
    total_tokens: number;
    total_cost: number;
  } | null>(null);
  const [recentCalls, setRecentCalls] = useState<LlmCall[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const fetchStats = async () => {
      setIsLoading(true);
      try {
        const [global, recent] = await Promise.all([
          getLlmCallStats('global').catch(() => null),
          getRecentLlmCalls(50).catch(() => [] as LlmCall[]),
        ]);
        setGlobalStats(global);
        setRecentCalls(recent);

        if (currentStory?.id) {
          const story = await getLlmCallStats(currentStory.id).catch(() => null);
          setStoryStats(story);
        } else {
          setStoryStats(null);
        }
      } catch (e) {
        console.warn('[UsageStats] fetch failed:', e);
      } finally {
        setIsLoading(false);
      }
    };

    fetchStats();
  }, [currentStory?.id]);

  if (isLoading) {
    return (
      <div className="p-8 flex items-center justify-center h-full">
        <Loader2 className="w-8 h-8 text-cinema-gold animate-spin" />
      </div>
    );
  }

  const formatTokens = (n: number) => {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
    return String(n);
  };

  const formatCost = (c: number) => {
    if (c >= 1) return `$${c.toFixed(2)}`;
    if (c > 0) return `$${c.toFixed(4)}`;
    return '$0';
  };

  return (
    <div className="p-8 space-y-6 animate-fade-in">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="font-display text-3xl font-bold text-white">用量统计</h1>
          <p className="text-gray-400">
            {currentStory ? `${currentStory.title} - ` : ''}LLM 调用与 Token 消耗概览
          </p>
        </div>
      </div>

      {/* Stats Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <Card>
          <CardContent className="p-5">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-xs text-gray-500 uppercase tracking-wider">总调用次数</p>
                <p className="text-2xl font-bold text-white mt-1">{globalStats?.count ?? 0}</p>
              </div>
              <Hash className="w-8 h-8 text-cinema-gold/40" />
            </div>
            {storyStats != null && (
              <p className="text-xs text-cinema-gold/60 mt-2">本故事: {storyStats.count}</p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-5">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-xs text-gray-500 uppercase tracking-wider">总 Token 数</p>
                <p className="text-2xl font-bold text-white mt-1">
                  {formatTokens(globalStats?.total_tokens ?? 0)}
                </p>
              </div>
              <Activity className="w-8 h-8 text-blue-400/40" />
            </div>
            {storyStats != null && (
              <p className="text-xs text-blue-400/60 mt-2">
                本故事: {formatTokens(storyStats.total_tokens)}
              </p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-5">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-xs text-gray-500 uppercase tracking-wider">预估费用</p>
                <p className="text-2xl font-bold text-white mt-1">
                  {formatCost(globalStats?.total_cost ?? 0)}
                </p>
              </div>
              <Coins className="w-8 h-8 text-green-400/40" />
            </div>
            {storyStats != null && (
              <p className="text-xs text-green-400/60 mt-2">
                本故事: {formatCost(storyStats.total_cost)}
              </p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-5">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-xs text-gray-500 uppercase tracking-wider">成功率</p>
                <p className="text-2xl font-bold text-white mt-1">
                  {recentCalls.length > 0
                    ? `${Math.round((recentCalls.filter(c => c.success).length / recentCalls.length) * 100)}%`
                    : 'N/A'}
                </p>
              </div>
              <BarChart3 className="w-8 h-8 text-purple-400/40" />
            </div>
            <p className="text-xs text-gray-600 mt-2">基于最近 {recentCalls.length} 次调用</p>
          </CardContent>
        </Card>
      </div>

      {/* Recent Calls Table */}
      <Card>
        <CardContent className="p-5">
          <div className="flex items-center gap-2 mb-4">
            <Clock className="w-4 h-4 text-gray-400" />
            <h2 className="font-display text-lg font-semibold text-white">最近调用</h2>
          </div>

          {recentCalls.length === 0 ? (
            <div className="text-center py-8 text-gray-500">暂无 LLM 调用记录</div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-cinema-700">
                    <th className="text-left py-2 px-3 text-gray-500 font-medium">用途</th>
                    <th className="text-left py-2 px-3 text-gray-500 font-medium">模型</th>
                    <th className="text-right py-2 px-3 text-gray-500 font-medium">Token</th>
                    <th className="text-right py-2 px-3 text-gray-500 font-medium">耗时</th>
                    <th className="text-center py-2 px-3 text-gray-500 font-medium">状态</th>
                    <th className="text-left py-2 px-3 text-gray-500 font-medium">时间</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-cinema-800">
                  {recentCalls.map(call => (
                    <tr key={call.id} className="hover:bg-cinema-800/30 transition-colors">
                      <td className="py-2 px-3 text-white/80">{call.purpose}</td>
                      <td className="py-2 px-3 text-gray-400">
                        {call.model_name || call.model_id}
                      </td>
                      <td className="py-2 px-3 text-right text-gray-400">
                        {call.total_tokens.toLocaleString()}
                      </td>
                      <td className="py-2 px-3 text-right text-gray-400">
                        {call.duration_ms >= 1000
                          ? `${(call.duration_ms / 1000).toFixed(1)}s`
                          : `${call.duration_ms}ms`}
                      </td>
                      <td className="py-2 px-3 text-center">
                        {call.success ? (
                          <CheckCircle className="w-4 h-4 text-green-400 mx-auto" />
                        ) : (
                          <XCircle className="w-4 h-4 text-red-400 mx-auto" />
                        )}
                      </td>
                      <td className="py-2 px-3 text-gray-500 text-xs">
                        {new Date(call.created_at).toLocaleString()}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
