import { useEffect, useState } from 'react';
import { GitBranch, Activity, Target, AlertTriangle } from 'lucide-react';
import { useAppStore } from '@/stores/appStore';
import {
  analyzeNarrativeStructure,
  getNarrativeEvents,
  getNarrativeThreads,
  type NarrativeStructureAct,
  type NarrativeEvent,
  type NarrativeThread,
} from '@/services/tauri';
import { createLogger } from '@/utils/logger';

const logger = createLogger('ui:NarrativeAnalysis');

export function NarrativeAnalysis() {
  const currentStory = useAppStore(s => s.currentStory);
  const [structure, setStructure] = useState<NarrativeStructureAct[]>([]);
  const [events, setEvents] = useState<NarrativeEvent[]>([]);
  const [threads, setThreads] = useState<NarrativeThread[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!currentStory?.id) return;
    loadData(currentStory.id);
  }, [currentStory?.id]);

  const loadData = async (storyId: string) => {
    setLoading(true);
    try {
      const [structRes, eventsRes, threadsRes] = await Promise.all([
        analyzeNarrativeStructure(storyId),
        getNarrativeEvents(storyId),
        getNarrativeThreads(storyId),
      ]);
      setStructure(structRes.structure || []);
      setEvents(eventsRes.events || []);
      setThreads(threadsRes.threads || []);
    } catch (e) {
      logger.error('加载叙事分析失败', { error: e });
    } finally {
      setLoading(false);
    }
  };

  if (!currentStory) {
    return (
      <div className="flex items-center justify-center h-full text-gray-500">
        请先在侧边栏选择一个故事
      </div>
    );
  }

  const actColors: Record<string, string> = {
    introduction: 'bg-emerald-500/20 border-emerald-500/40',
    development: 'bg-blue-500/20 border-blue-500/40',
    turn: 'bg-amber-500/20 border-amber-500/40',
    resolution: 'bg-rose-500/20 border-rose-500/40',
  };

  const actLabels: Record<string, string> = {
    introduction: '起',
    development: '承',
    turn: '转',
    resolution: '合',
  };

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center gap-3">
        <GitBranch className="w-6 h-6 text-cinema-gold" />
        <h1 className="text-2xl font-bold text-white">叙事分析</h1>
        {loading && <span className="text-sm text-gray-500">加载中...</span>}
      </div>

      {/* 幕级结构 */}
      <section className="space-y-3">
        <h2 className="text-lg font-semibold text-white flex items-center gap-2">
          <Target className="w-4 h-4" />
          幕级结构
        </h2>
        {structure.length === 0 ? (
          <p className="text-gray-500 text-sm">暂无分析数据。保存章节后将自动分析。</p>
        ) : (
          <div className="grid grid-cols-4 gap-3">
            {structure.map(act => (
              <div
                key={act.act_number}
                className={`p-4 rounded-lg border ${actColors[act.act_type] || 'bg-gray-500/20 border-gray-500/40'}`}
              >
                <div className="text-2xl font-bold text-white mb-1">
                  {actLabels[act.act_type] || act.act_type}
                </div>
                <div className="text-sm text-gray-400">
                  第 {act.start_chapter} — {act.end_chapter} 章
                </div>
                {act.summary && (
                  <div className="text-xs text-gray-500 mt-2 line-clamp-2">{act.summary}</div>
                )}
              </div>
            ))}
          </div>
        )}
      </section>

      {/* 事件强度 */}
      <section className="space-y-3">
        <h2 className="text-lg font-semibold text-white flex items-center gap-2">
          <Activity className="w-4 h-4" />
          事件强度 ({events.length})
        </h2>
        {events.length === 0 ? (
          <p className="text-gray-500 text-sm">暂无事件数据。</p>
        ) : (
          <div className="space-y-2">
            {events.slice(0, 20).map(ev => (
              <div
                key={ev.scene_id}
                className="flex items-center gap-3 bg-cinema-800/50 rounded px-3 py-2"
              >
                <div className="text-sm text-gray-400 w-16">第{ev.scene_number}章</div>
                <div className="flex-1 text-sm text-white truncate">{ev.title || '未命名场景'}</div>
                <div className="w-32 h-2 bg-cinema-900 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-cinema-gold rounded-full"
                    style={{ width: `${((ev.intensity || 0.5) * 100).toFixed(0)}%` }}
                  />
                </div>
                <div className="text-xs text-gray-500 w-12 text-right">
                  {(ev.intensity || 0).toFixed(1)}
                </div>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* 活跃线索 */}
      <section className="space-y-3">
        <h2 className="text-lg font-semibold text-white flex items-center gap-2">
          <AlertTriangle className="w-4 h-4" />
          活跃线索 ({threads.length})
        </h2>
        {threads.length === 0 ? (
          <p className="text-gray-500 text-sm">暂无活跃线索。</p>
        ) : (
          <div className="grid grid-cols-2 gap-3">
            {threads.map((thread, idx) => (
              <div key={idx} className="bg-cinema-800/50 rounded-lg p-3">
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-xs px-2 py-0.5 rounded bg-cinema-700 text-gray-300">
                    {thread.type}
                  </span>
                  {thread.risk_score !== undefined && thread.risk_score > 0.5 && (
                    <span className="text-xs text-amber-400">高风险</span>
                  )}
                </div>
                <div className="text-sm text-white">{thread.content}</div>
                <div className="text-xs text-gray-500 mt-1">状态: {thread.status}</div>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
