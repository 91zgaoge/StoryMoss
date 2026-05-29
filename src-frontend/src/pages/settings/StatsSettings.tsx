import { useState, useEffect } from 'react';
import { Loader2, RefreshCw } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { getFeatureUsageStats } from '@/services/tauri';
import toast from 'react-hot-toast';

export function StatsSettings() {
  const [stats, setStats] = useState<Array<{ feature_id: string; action: string; count: number }>>(
    []
  );
  const [loading, setLoading] = useState(false);

  const loadStats = async () => {
    setLoading(true);
    try {
      const data = await getFeatureUsageStats(30);
      setStats(data);
    } catch (e) {
      toast.error('加载统计数据失败');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadStats();
  }, []);

  const features = [
    { id: 'story_contract', name: '合同驱动' },
    { id: 'memory_pack', name: '记忆编排' },
    { id: 'reading_power', name: '追读力' },
    { id: 'anti_ai_review', name: 'Anti-AI 审查' },
    { id: 'genre_template', name: '体裁模板' },
  ];

  const getCount = (featureId: string, action?: string) => {
    return stats
      .filter(s => s.feature_id === featureId && (action ? s.action === action : true))
      .reduce((sum, s) => sum + s.count, 0);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold text-white">功能使用统计（最近 30 天）</h3>
        <Button size="sm" onClick={loadStats} disabled={loading}>
          {loading ? (
            <Loader2 className="w-3 h-3 animate-spin" />
          ) : (
            <RefreshCw className="w-3 h-3" />
          )}
          <span className="ml-1">刷新</span>
        </Button>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {features.map(f => {
          const opened = getCount(f.id, 'opened');
          const executed = getCount(f.id, 'executed');
          return (
            <Card key={f.id}>
              <CardContent className="p-4">
                <p className="text-white font-medium">{f.name}</p>
                <div className="mt-3 space-y-1">
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-gray-400">打开次数</span>
                    <span className="text-white">{opened}</span>
                  </div>
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-gray-400">执行次数</span>
                    <span className="text-white">{executed}</span>
                  </div>
                  <div className="w-full h-2 bg-cinema-800 rounded-full mt-2 overflow-hidden">
                    <div
                      className="h-full bg-cinema-gold rounded-full transition-all"
                      style={{ width: `${Math.min((opened + executed) * 5, 100)}%` }}
                    />
                  </div>
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>
    </div>
  );
}
