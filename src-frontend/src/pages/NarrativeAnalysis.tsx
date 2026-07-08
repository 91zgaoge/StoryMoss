import { useEffect, useState } from 'react';
import { GitBranch, Activity, Target, AlertTriangle, HeartPulse } from 'lucide-react';
import { useAppStore } from '@/stores/appStore';
import {
  analyzeNarrativeStructure,
  getNarrativeEvents,
  getNarrativeThreads,
  type NarrativeStructureAct,
  type NarrativeEvent,
  type NarrativeThread,
} from '@/services/tauri';
import { getStorySummaries } from '@/services/api/knowledge';
import { createLogger } from '@/utils/logger';

const logger = createLogger('ui:NarrativeAnalysis');

interface ReadingPowerPoint {
  chapter: number;
  score: number;
}

function ReadingPowerChart({ data }: { data: ReadingPowerPoint[] }) {
  if (data.length === 0) return null;
  const width = 600;
  const height = 160;
  const padding = { top: 10, right: 20, bottom: 30, left: 30 };
  const chartWidth = width - padding.left - padding.right;
  const chartHeight = height - padding.top - padding.bottom;
  const maxScore = Math.max(100, ...data.map(d => d.score));
  const minChapter = Math.min(...data.map(d => d.chapter));
  const maxChapter = Math.max(...data.map(d => d.chapter));
  const chapterRange = Math.max(1, maxChapter - minChapter);

  const xFor = (chapter: number) =>
    padding.left + ((chapter - minChapter) / chapterRange) * chartWidth;
  const yFor = (score: number) => padding.top + chartHeight - (score / maxScore) * chartHeight;

  const pathD = data
    .map((d, i) => `${i === 0 ? 'M' : 'L'} ${xFor(d.chapter)} ${yFor(d.score)}`)
    .join(' ');
  const areaD = `${pathD} L ${xFor(data[data.length - 1].chapter)} ${padding.top + chartHeight} L ${xFor(data[0].chapter)} ${padding.top + chartHeight} Z`;

  const ticks = 5;
  const yTicks = Array.from({ length: ticks + 1 }, (_, i) => (maxScore / ticks) * i);

  return (
    <svg viewBox={`0 0 ${width} ${height}`} className="w-full h-48">
      {/* Grid lines */}
      {yTicks.map((tick, i) => (
        <g key={i}>
          <line
            x1={padding.left}
            y1={yFor(tick)}
            x2={padding.left + chartWidth}
            y2={yFor(tick)}
            stroke="#374151"
            strokeDasharray="4 4"
            strokeWidth={1}
          />
          <text
            x={padding.left - 8}
            y={yFor(tick) + 4}
            fill="#9CA3AF"
            fontSize={10}
            textAnchor="end"
          >
            {Math.round(tick)}
          </text>
        </g>
      ))}

      {/* Area */}
      <path d={areaD} fill="rgba(251, 191, 36, 0.15)" />

      {/* Line */}
      <path d={pathD} fill="none" stroke="#FBBF24" strokeWidth={2} />

      {/* Data points */}
      {data.map(d => (
        <g key={d.chapter}>
          <circle cx={xFor(d.chapter)} cy={yFor(d.score)} r={3} fill="#FBBF24" />
          <text
            x={xFor(d.chapter)}
            y={padding.top + chartHeight + 16}
            fill="#9CA3AF"
            fontSize={9}
            textAnchor="middle"
          >
            第{d.chapter}章
          </text>
        </g>
      ))}
    </svg>
  );
}

interface InsightReport {
  overall_health: number;
  chapter_range: [number, number];
  evaluated_at: string;
  reading_power_trend: Array<{
    chapter: number;
    score: number;
    hook_strength: string;
    coolpoint_count: number;
    micropayoff_count: number;
    debt_balance: number;
  }>;
  chase_debt: { total_amount: number; active_count: number; overdue_count: number };
  unresolved_annotations: { total: number; high_severity: number; ai_audit: number };
}

export function NarrativeAnalysis() {
  const currentStory = useAppStore(s => s.currentStory);
  const [structure, setStructure] = useState<NarrativeStructureAct[]>([]);
  const [events, setEvents] = useState<NarrativeEvent[]>([]);
  const [threads, setThreads] = useState<NarrativeThread[]>([]);
  const [insight, setInsight] = useState<InsightReport | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!currentStory?.id) return;
    loadData(currentStory.id);
  }, [currentStory?.id]);

  const loadData = async (storyId: string) => {
    setLoading(true);
    try {
      const [structRes, eventsRes, threadsRes, summaries] = await Promise.all([
        analyzeNarrativeStructure(storyId),
        getNarrativeEvents(storyId),
        getNarrativeThreads(storyId),
        getStorySummaries(storyId),
      ]);
      setStructure(structRes.structure || []);
      setEvents(eventsRes.events || []);
      setThreads(threadsRes.threads || []);
      // 筛选最新的 deep_insight 报告
      const insightSummary = summaries
        ?.filter(s => s.summary_type === 'deep_insight')
        .sort((a, b) => b.updated_at.localeCompare(a.updated_at))[0];
      if (insightSummary) {
        try {
          setInsight(JSON.parse(insightSummary.content));
        } catch {
          setInsight(null);
        }
      } else {
        setInsight(null);
      }
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

      {/* 深度洞察（时间线 3） */}
      <section className="space-y-3">
        <h2 className="text-lg font-semibold text-white flex items-center gap-2">
          <HeartPulse className="w-4 h-4" />
          深度洞察
        </h2>
        {!insight ? (
          <p className="text-gray-500 text-sm">
            暂无洞察报告。每生成 5 段正文后自动生成，或在此期间无足够数据。
          </p>
        ) : (
          <div className="space-y-4">
            {/* 整体健康度 */}
            <div className="bg-cinema-800/50 rounded-lg p-4">
              <div className="flex items-center justify-between mb-2">
                <span className="text-sm text-gray-400">整体健康度</span>
                <span className="text-xs text-gray-500">
                  第 {insight.chapter_range[0]}—{insight.chapter_range[1]} 章 ·{' '}
                  {new Date(insight.evaluated_at).toLocaleString('zh-CN')}
                </span>
              </div>
              <div className="flex items-center gap-3">
                <span
                  className={`text-3xl font-bold ${
                    insight.overall_health >= 70
                      ? 'text-emerald-400'
                      : insight.overall_health >= 40
                        ? 'text-amber-400'
                        : 'text-red-400'
                  }`}
                >
                  {insight.overall_health.toFixed(0)}
                </span>
                <span className="text-sm text-gray-500">/ 100</span>
                <div className="flex-1 h-3 bg-cinema-900 rounded-full overflow-hidden ml-2">
                  <div
                    className={`h-full rounded-full ${
                      insight.overall_health >= 70
                        ? 'bg-emerald-500'
                        : insight.overall_health >= 40
                          ? 'bg-amber-500'
                          : 'bg-red-500'
                    }`}
                    style={{ width: `${insight.overall_health}%` }}
                  />
                </div>
              </div>
            </div>

            {/* 追读力趋势 */}
            {insight.reading_power_trend.length > 0 && (
              <div className="bg-cinema-800/50 rounded-lg p-4">
                <div className="text-sm text-gray-400 mb-3">追读力趋势</div>
                <ReadingPowerChart data={insight.reading_power_trend} />
              </div>
            )}

            {/* 债务 + annotation 汇总 */}
            <div className="grid grid-cols-2 gap-3">
              <div className="bg-cinema-800/50 rounded-lg p-4">
                <div className="text-sm text-gray-400 mb-2">追读债务</div>
                <div className="text-2xl font-bold text-white">
                  {insight.chase_debt.total_amount.toFixed(1)}
                </div>
                <div className="text-xs text-gray-500 mt-1">
                  {insight.chase_debt.active_count} 条活跃
                  {insight.chase_debt.overdue_count > 0 && (
                    <span className="text-red-400 ml-1">
                      · {insight.chase_debt.overdue_count} 条逾期
                    </span>
                  )}
                </div>
              </div>
              <div className="bg-cinema-800/50 rounded-lg p-4">
                <div className="text-sm text-gray-400 mb-2">未处理标注</div>
                <div className="text-2xl font-bold text-white">
                  {insight.unresolved_annotations.total}
                </div>
                <div className="text-xs text-gray-500 mt-1">
                  {insight.unresolved_annotations.high_severity} 条高优先级
                  <span className="text-amber-400 ml-1">
                    · {insight.unresolved_annotations.ai_audit} 条 AI 审计
                  </span>
                </div>
              </div>
            </div>
          </div>
        )}
      </section>
    </div>
  );
}
