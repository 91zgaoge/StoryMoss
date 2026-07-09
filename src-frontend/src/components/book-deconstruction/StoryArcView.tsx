import {
  GitBranch,
  Mountain,
  Zap,
  TrendingUp,
  Layers,
  Activity,
  BookOpen,
  BarChart3,
} from 'lucide-react';
import type { ReferenceBook, ReferenceScene } from '@/types/book-deconstruction';

interface StoryArcViewProps {
  book: ReferenceBook;
  scenes: ReferenceScene[];
}

interface ActData {
  act_number: number;
  act_type: string;
  start_chapter: number;
  end_chapter: number;
}

/** Legacy BookAnalyzer shape */
interface LegacyStoryArc {
  main_arc: string;
  sub_arcs: string[];
  climaxes: string[];
  turning_points: string[];
}

/** AnalysisPipeline OutlineAct[] persisted into story_arc (v0.26.46+) */
interface OutlineActArc {
  act_number: number;
  title: string;
  summary: string;
  key_plot_points?: string[];
}

function normalizeStoryArc(raw: string): LegacyStoryArc | null {
  try {
    const parsed: unknown = JSON.parse(raw);
    if (Array.isArray(parsed)) {
      const acts = parsed as OutlineActArc[];
      if (acts.length === 0) return null;
      const main = acts[0];
      const subs = acts
        .slice(1)
        .map(a => a.summary)
        .filter(Boolean);
      const turning = main.key_plot_points ?? [];
      const last = acts[acts.length - 1];
      const climaxes = acts.length > 1 ? (last.key_plot_points ?? []).filter(Boolean) : [];
      return {
        main_arc: main.summary || main.title || '',
        sub_arcs: subs,
        climaxes,
        turning_points: turning,
      };
    }
    if (parsed && typeof parsed === 'object' && 'main_arc' in parsed) {
      return parsed as LegacyStoryArc;
    }
    return null;
  } catch {
    return null;
  }
}

export function StoryArcView({ book, scenes }: StoryArcViewProps) {
  const arc = book.story_arc ? normalizeStoryArc(book.story_arc) : null;

  // LitSeg: 解析叙事幕结构
  const parseActs = (): ActData[] | null => {
    if (!book.analyzed_structure_json) return null;
    try {
      return JSON.parse(book.analyzed_structure_json) as ActData[];
    } catch {
      return null;
    }
  };

  const acts = parseActs();

  // LitSeg: 计算每个场景在整体中的位置（0-100%）
  const getScenePosition = (seq: number): number => {
    if (scenes.length <= 1) return 0;
    return ((seq - 1) / (scenes.length - 1)) * 100;
  };

  // LitSeg: 场景强度颜色映射
  const getIntensityColor = (intensity?: number): string => {
    if (!intensity) return 'bg-gray-700';
    if (intensity >= 0.8) return 'bg-red-500';
    if (intensity >= 0.6) return 'bg-orange-500';
    if (intensity >= 0.4) return 'bg-yellow-500';
    if (intensity >= 0.2) return 'bg-blue-400';
    return 'bg-gray-600';
  };

  // LitSeg: 幕类型中文映射
  const actTypeLabels: Record<string, string> = {
    Setup: '起（铺设）',
    Confrontation: '承（对抗）',
    RisingAction: '转（上升）',
    Climax: '高潮',
    Resolution: '合（结局）',
    Exposition: ' exposition',
    Development: '发展',
    Transition: '转折',
  };

  const actColors: Record<number, string> = {
    1: 'border-emerald-500/50 bg-emerald-500/10',
    2: 'border-blue-500/50 bg-blue-500/10',
    3: 'border-amber-500/50 bg-amber-500/10',
    4: 'border-red-500/50 bg-red-500/10',
    5: 'border-purple-500/50 bg-purple-500/10',
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2 mb-4">
        <GitBranch className="w-5 h-5 text-cinema-gold" />
        <h3 className="text-lg font-medium text-white">故事线</h3>
      </div>

      {/* ====== LitSeg: 幕结构图 ====== */}
      {acts && acts.length > 0 && (
        <div className="bg-cinema-900 border border-cinema-800 rounded-xl p-4">
          <div className="flex items-center gap-2 mb-4">
            <Layers className="w-4 h-4 text-cinema-gold" />
            <h4 className="text-sm font-medium text-cinema-gold">叙事幕结构</h4>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5 gap-3">
            {acts.map(act => (
              <div
                key={act.act_number}
                className={`rounded-lg border p-3 ${actColors[act.act_number] || 'border-gray-600 bg-gray-800/30'}`}
              >
                <div className="flex items-center justify-between mb-1">
                  <span className="text-xs font-medium text-gray-400">第{act.act_number}幕</span>
                  <BookOpen className="w-3 h-3 text-gray-500" />
                </div>
                <div className="text-sm font-medium text-white mb-1">
                  {actTypeLabels[act.act_type] || act.act_type}
                </div>
                <div className="text-xs text-gray-500">
                  第{act.start_chapter}章 - 第{act.end_chapter}章
                </div>
                <div className="mt-2 h-1.5 rounded-full bg-gray-800 overflow-hidden">
                  <div
                    className="h-full rounded-full bg-cinema-gold/60"
                    style={{
                      width: `${Math.max(5, ((act.end_chapter - act.start_chapter + 1) / Math.max(acts[acts.length - 1].end_chapter, 1)) * 100)}%`,
                    }}
                  />
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* ====== LitSeg: 场景强度时间线 ====== */}
      {scenes.some(s => s.narrative_intensity !== undefined && s.narrative_intensity !== null) && (
        <div className="bg-cinema-900 border border-cinema-800 rounded-xl p-4">
          <div className="flex items-center gap-2 mb-4">
            <BarChart3 className="w-4 h-4 text-cinema-gold" />
            <h4 className="text-sm font-medium text-cinema-gold">场景叙事强度时间线</h4>
          </div>
          <div className="space-y-2">
            {scenes
              .filter(s => s.narrative_intensity !== undefined && s.narrative_intensity !== null)
              .map(scene => (
                <div key={scene.id} className="flex items-center gap-3">
                  <div className="w-8 text-xs text-gray-500 text-right shrink-0">
                    {scene.sequence_number}
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-0.5">
                      <span className="text-xs text-gray-400 truncate">
                        {scene.title || `第${scene.sequence_number}章`}
                      </span>
                      {scene.act_number ? (
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-gray-800 text-gray-500">
                          第{scene.act_number}幕
                        </span>
                      ) : null}
                    </div>
                    <div className="h-2 rounded-full bg-gray-800 overflow-hidden">
                      <div
                        className={`h-full rounded-full transition-all ${getIntensityColor(scene.narrative_intensity)}`}
                        style={{ width: `${(scene.narrative_intensity ?? 0) * 100}%` }}
                      />
                    </div>
                  </div>
                  <div className="w-10 text-xs text-gray-500 text-right shrink-0">
                    {((scene.narrative_intensity ?? 0) * 100).toFixed(0)}%
                  </div>
                </div>
              ))}
          </div>
        </div>
      )}

      {/* ====== LitSeg: 场景情感分布 ====== */}
      {scenes.some(s => s.narrative_sentiment !== undefined && s.narrative_sentiment !== null) && (
        <div className="bg-cinema-900 border border-cinema-800 rounded-xl p-4">
          <div className="flex items-center gap-2 mb-4">
            <Activity className="w-4 h-4 text-cinema-gold" />
            <h4 className="text-sm font-medium text-cinema-gold">场景情感分布</h4>
          </div>
          <div className="relative h-16 bg-gray-800/50 rounded-lg overflow-hidden">
            {scenes
              .filter(s => s.narrative_sentiment !== undefined && s.narrative_sentiment !== null)
              .map((scene, idx, arr) => {
                const sentiment = scene.narrative_sentiment ?? 0;
                const color =
                  sentiment > 0.3
                    ? 'bg-emerald-500'
                    : sentiment > 0
                      ? 'bg-emerald-500/50'
                      : sentiment > -0.3
                        ? 'bg-gray-500'
                        : sentiment > -0.6
                          ? 'bg-orange-500/50'
                          : 'bg-red-500';
                return (
                  <div
                    key={scene.id}
                    className={`absolute bottom-0 ${color} rounded-t-sm`}
                    style={{
                      left: `${(idx / Math.max(arr.length - 1, 1)) * 100}%`,
                      width: `${100 / Math.max(arr.length, 1)}%`,
                      height: `${Math.abs(sentiment) * 100}%`,
                      minHeight: '4px',
                      transform: 'translateX(-50%)',
                    }}
                    title={`第${scene.sequence_number}章: ${sentiment > 0 ? '正向' : sentiment < 0 ? '负向' : '中性'} ${sentiment.toFixed(2)}`}
                  />
                );
              })}
          </div>
          <div className="flex justify-between text-[10px] text-gray-600 mt-1">
            <span>负向</span>
            <span>中性</span>
            <span>正向</span>
          </div>
        </div>
      )}

      {/* 主线 */}
      {arc?.main_arc && (
        <div className="bg-cinema-900 border border-cinema-800 rounded-xl p-4">
          <h4 className="text-sm font-medium text-cinema-gold mb-2">主线故事</h4>
          <p className="text-sm text-gray-300 leading-relaxed">{arc.main_arc}</p>
        </div>
      )}

      {/* 剧情概要 */}
      {book.plot_summary && (
        <div className="bg-cinema-900 border border-cinema-800 rounded-xl p-4">
          <h4 className="text-sm font-medium text-cinema-gold mb-2">剧情概要</h4>
          <p className="text-sm text-gray-300 leading-relaxed">{book.plot_summary}</p>
        </div>
      )}

      {/* 高潮点 */}
      {arc?.climaxes && arc.climaxes.length > 0 && (
        <div className="bg-cinema-900 border border-cinema-800 rounded-xl p-4">
          <div className="flex items-center gap-2 mb-3">
            <Mountain className="w-4 h-4 text-red-400" />
            <h4 className="text-sm font-medium text-red-400">高潮点</h4>
          </div>
          <div className="space-y-2">
            {arc.climaxes.map((climax, i) => (
              <div key={i} className="flex items-start gap-2">
                <span className="text-xs text-red-400/60 mt-0.5">{i + 1}.</span>
                <p className="text-sm text-gray-300">{climax}</p>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 转折点 */}
      {arc?.turning_points && arc.turning_points.length > 0 && (
        <div className="bg-cinema-900 border border-cinema-800 rounded-xl p-4">
          <div className="flex items-center gap-2 mb-3">
            <Zap className="w-4 h-4 text-yellow-400" />
            <h4 className="text-sm font-medium text-yellow-400">转折点</h4>
          </div>
          <div className="space-y-2">
            {arc.turning_points.map((point, i) => (
              <div key={i} className="flex items-start gap-2">
                <span className="text-xs text-yellow-400/60 mt-0.5">{i + 1}.</span>
                <p className="text-sm text-gray-300">{point}</p>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 支线 */}
      {arc?.sub_arcs && arc.sub_arcs.length > 0 && (
        <div className="bg-cinema-900 border border-cinema-800 rounded-xl p-4">
          <h4 className="text-sm font-medium text-blue-400 mb-3">支线故事</h4>
          <div className="space-y-2">
            {arc.sub_arcs.map((sub, i) => (
              <div key={i} className="flex items-start gap-2">
                <span className="text-xs text-blue-400/60 mt-0.5">{i + 1}.</span>
                <p className="text-sm text-gray-300">{sub}</p>
              </div>
            ))}
          </div>
        </div>
      )}

      {!arc && !book.plot_summary && !acts && (
        <div className="text-center py-8 text-gray-500">暂无故事线数据</div>
      )}
    </div>
  );
}
