/**
 * 数据洞察 — 合并用量统计 / 写作统计 / 功能使用（v0.26.39）
 */
import { useEffect } from 'react';
import { BarChart3, PenLine, LayoutGrid } from 'lucide-react';
import { cn } from '@/utils/cn';
import { useAppStore } from '@/stores/appStore';
import { UsageStats } from './UsageStats';
import { WritingStats } from './WritingStats';
import { StatsSettings } from './settings/StatsSettings';
import { logFeatureUsage } from '@/services/tauri';

type InsightsTab = 'usage' | 'writing' | 'features';

const TABS: { id: InsightsTab; label: string; icon: React.ElementType }[] = [
  { id: 'usage', label: '用量', icon: BarChart3 },
  { id: 'writing', label: '写作', icon: PenLine },
  { id: 'features', label: '功能使用', icon: LayoutGrid },
];

export function Insights() {
  const insightsTab = useAppStore(s => s.insightsTab);
  const setInsightsTab = useAppStore(s => s.setInsightsTab);

  useEffect(() => {
    if (insightsTab === 'features') {
      logFeatureUsage('feature_stats', 'opened');
    }
  }, [insightsTab]);

  return (
    <div className="p-8 space-y-6 animate-fade-in">
      <div>
        <h1 className="font-display text-3xl font-bold text-white">数据洞察</h1>
        <p className="text-gray-400 mt-1">用量、写作进度与功能使用一览</p>
      </div>

      <div
        className="flex items-center gap-2 border-b border-cinema-800 pb-4"
        data-testid="insights-tabs"
      >
        {TABS.map(tab => {
          const Icon = tab.icon;
          const active = insightsTab === tab.id;
          return (
            <button
              key={tab.id}
              type="button"
              onClick={() => setInsightsTab(tab.id)}
              className={cn(
                'flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors',
                active
                  ? 'bg-cinema-gold text-black'
                  : 'text-gray-400 hover:text-white hover:bg-cinema-800'
              )}
            >
              <Icon className="w-4 h-4" />
              {tab.label}
            </button>
          );
        })}
      </div>

      {insightsTab === 'usage' && <UsageStats embedded />}
      {insightsTab === 'writing' && <WritingStats embedded />}
      {insightsTab === 'features' && <StatsSettings />}
    </div>
  );
}
