import React, { useState, useEffect, useCallback } from 'react';
import { RefreshCw, Save, Undo2, Radar } from 'lucide-react';
import { cn } from '@/utils/cn';
import { getLatestStyleSnapshot } from '@/services/tauri';
import type { StyleSnapshot } from '@/services/tauri';
import { createLogger } from '@/utils/logger';
import toast from 'react-hot-toast';

const styleLogger = createLogger('ui:StyleDnaRadar');

const DIMENSIONS = [
  { key: 'sentence_length', label: '句长', color: '#f59e0b' },
  { key: 'dialogue_ratio', label: '对话比', color: '#3b82f6' },
  { key: 'metaphor_density', label: '隐喻密度', color: '#8b5cf6' },
  { key: 'inner_monologue_ratio', label: '独白比', color: '#10b981' },
  { key: 'emotion_density', label: '情感密度', color: '#ef4444' },
  { key: 'rhythm_score', label: '节奏感', color: '#06b6d4' },
] as const;

type DimensionKey = (typeof DIMENSIONS)[number]['key'];

interface StyleDnaRadarProps {
  storyId: string;
  onConstraintChange?: (constraints: Record<DimensionKey, number>) => void;
  className?: string;
}

function hexToRgb(hex: string): string {
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
  if (!result) return '255,255,255';
  return `${parseInt(result[1], 16)},${parseInt(result[2], 16)},${parseInt(result[3], 16)}`;
}

export const StyleDnaRadar: React.FC<StyleDnaRadarProps> = ({
  storyId,
  onConstraintChange,
  className,
}) => {
  const [snapshot, setSnapshot] = useState<StyleSnapshot | null>(null);
  const [values, setValues] = useState<Record<DimensionKey, number>>({
    sentence_length: 0.5,
    dialogue_ratio: 0.5,
    metaphor_density: 0.5,
    inner_monologue_ratio: 0.5,
    emotion_density: 0.5,
    rhythm_score: 0.5,
  });
  const [originalValues, setOriginalValues] = useState<Record<DimensionKey, number> | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [hasChanges, setHasChanges] = useState(false);

  const loadSnapshot = useCallback(async () => {
    if (!storyId) return;
    setIsLoading(true);
    try {
      const data = await getLatestStyleSnapshot(storyId);
      if (data) {
        setSnapshot(data);
        const newValues: Record<DimensionKey, number> = {
          sentence_length: Math.min(1, Math.max(0, data.sentence_length)),
          dialogue_ratio: Math.min(1, Math.max(0, data.dialogue_ratio)),
          metaphor_density: Math.min(1, Math.max(0, data.metaphor_density)),
          inner_monologue_ratio: Math.min(1, Math.max(0, data.inner_monologue_ratio)),
          emotion_density: Math.min(1, Math.max(0, data.emotion_density)),
          rhythm_score: Math.min(1, Math.max(0, data.rhythm_score)),
        };
        setValues(newValues);
        setOriginalValues({ ...newValues });
        setHasChanges(false);
      } else {
        // 无快照，使用默认值
        const defaults: Record<DimensionKey, number> = {
          sentence_length: 0.5,
          dialogue_ratio: 0.5,
          metaphor_density: 0.5,
          inner_monologue_ratio: 0.5,
          emotion_density: 0.5,
          rhythm_score: 0.5,
        };
        setValues(defaults);
        setOriginalValues({ ...defaults });
        setHasChanges(false);
      }
    } catch (error) {
      styleLogger.error('Failed to load style snapshot', { error });
    } finally {
      setIsLoading(false);
    }
  }, [storyId]);

  useEffect(() => {
    loadSnapshot();
  }, [loadSnapshot]);

  const handleValueChange = (key: DimensionKey, val: number) => {
    const clamped = Math.min(1, Math.max(0, val));
    setValues(prev => {
      const next = { ...prev, [key]: clamped };
      setHasChanges(
        originalValues !== null &&
          Object.keys(next).some(k => next[k as DimensionKey] !== originalValues[k as DimensionKey])
      );
      return next;
    });
  };

  const handleReset = () => {
    if (originalValues) {
      setValues({ ...originalValues });
      setHasChanges(false);
    }
  };

  const handleSave = () => {
    onConstraintChange?.(values);
    setOriginalValues({ ...values });
    setHasChanges(false);
    toast.success('StyleDNA 约束已保存，AI 生成将遵循此配置');
  };

  // SVG Radar Chart
  const size = 240;
  const center = size / 2;
  const radius = size * 0.38;
  const angleStep = (Math.PI * 2) / DIMENSIONS.length;

  const getPoint = (idx: number, val: number) => {
    const angle = idx * angleStep - Math.PI / 2;
    const r = radius * val;
    return {
      x: center + r * Math.cos(angle),
      y: center + r * Math.sin(angle),
    };
  };

  const gridLevels = [0.2, 0.4, 0.6, 0.8, 1.0];

  const dataPoints = DIMENSIONS.map((d, i) => getPoint(i, values[d.key]));
  const dataPath =
    dataPoints.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x} ${p.y}`).join(' ') + ' Z';

  return (
    <div className={cn('bg-[#1a1a2e] border border-white/5 rounded-xl overflow-hidden', className)}>
      {/* Header */}
      <div className="px-4 py-3 border-b border-white/5 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Radar className="w-4 h-4 text-cinema-gold" />
          <h3 className="text-sm font-semibold text-white/90">StyleDNA 雷达图</h3>
        </div>
        <div className="flex items-center gap-1.5">
          <button
            onClick={loadSnapshot}
            disabled={isLoading}
            className="p-1.5 rounded-lg hover:bg-white/5 text-white/40 hover:text-white/70 transition-colors disabled:opacity-50"
            title="刷新"
          >
            <RefreshCw className={cn('w-3.5 h-3.5', isLoading && 'animate-spin')} />
          </button>
          <button
            onClick={handleReset}
            disabled={!hasChanges}
            className="p-1.5 rounded-lg hover:bg-white/5 text-white/40 hover:text-white/70 transition-colors disabled:opacity-30"
            title="重置"
          >
            <Undo2 className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={handleSave}
            disabled={!hasChanges}
            className={cn(
              'p-1.5 rounded-lg transition-colors disabled:opacity-30',
              hasChanges
                ? 'bg-cinema-gold/20 text-cinema-gold hover:bg-cinema-gold/30'
                : 'text-white/40 hover:text-white/70'
            )}
            title="保存约束"
          >
            <Save className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Radar Chart */}
      <div className="p-4 flex flex-col items-center">
        <div className="relative">
          <svg width={size} height={size} className="block">
            {/* Background grid */}
            {gridLevels.map(level => {
              const points = DIMENSIONS.map((_, i) => {
                const p = getPoint(i, level);
                return `${p.x},${p.y}`;
              }).join(' ');
              return (
                <polygon
                  key={level}
                  points={points}
                  fill="none"
                  stroke="rgba(255,255,255,0.06)"
                  strokeWidth={1}
                />
              );
            })}

            {/* Axis lines */}
            {DIMENSIONS.map((_, i) => {
              const p = getPoint(i, 1);
              return (
                <line
                  key={`axis-${i}`}
                  x1={center}
                  y1={center}
                  x2={p.x}
                  y2={p.y}
                  stroke="rgba(255,255,255,0.06)"
                  strokeWidth={1}
                />
              );
            })}

            {/* Data area */}
            <polygon
              points={dataPoints.map(p => `${p.x},${p.y}`).join(' ')}
              fill="rgba(245,158,11,0.15)"
              stroke="#f59e0b"
              strokeWidth={2}
              strokeLinejoin="round"
            />

            {/* Data points */}
            {dataPoints.map((p, i) => (
              <circle
                key={`point-${i}`}
                cx={p.x}
                cy={p.y}
                r={3.5}
                fill={DIMENSIONS[i].color}
                stroke="#1a1a2e"
                strokeWidth={2}
              />
            ))}

            {/* Labels */}
            {DIMENSIONS.map((dim, i) => {
              const p = getPoint(i, 1.18);
              const isRight = p.x > center;
              const isTop = p.y < center;
              return (
                <text
                  key={`label-${i}`}
                  x={p.x}
                  y={p.y}
                  textAnchor={isRight ? 'start' : 'end'}
                  dominantBaseline={isTop ? 'auto' : 'hanging'}
                  fill="rgba(255,255,255,0.5)"
                  fontSize={10}
                  fontFamily="system-ui, sans-serif"
                >
                  {dim.label}
                </text>
              );
            })}
          </svg>

          {/* Center value */}
          <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
            <div className="text-center">
              <div className="text-lg font-bold text-cinema-gold">
                {((Object.values(values).reduce((a, b) => a + b, 0) / 6) * 100).toFixed(0)}
              </div>
              <div className="text-[9px] text-white/30">综合分</div>
            </div>
          </div>
        </div>

        {/* Dimension Sliders */}
        <div className="w-full mt-4 space-y-2.5">
          {DIMENSIONS.map(dim => {
            const val = values[dim.key];
            const original = originalValues?.[dim.key];
            const isModified = original !== undefined && Math.abs(val - original) > 0.001;

            return (
              <div key={dim.key} className="space-y-1">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-1.5">
                    <div className="w-2 h-2 rounded-full" style={{ backgroundColor: dim.color }} />
                    <span
                      className={cn(
                        'text-[11px]',
                        isModified ? 'text-cinema-gold font-medium' : 'text-white/50'
                      )}
                    >
                      {dim.label}
                    </span>
                  </div>
                  <span className="text-[11px] text-white/40 tabular-nums">
                    {(val * 100).toFixed(0)}%
                  </span>
                </div>
                <input
                  type="range"
                  min={0}
                  max={1}
                  step={0.01}
                  value={val}
                  onChange={e => handleValueChange(dim.key, parseFloat(e.target.value))}
                  className="w-full h-1 bg-white/5 rounded-full appearance-none cursor-pointer accent-cinema-gold"
                  style={{
                    background: `linear-gradient(to right, ${dim.color} 0%, ${dim.color} ${val * 100}%, rgba(255,255,255,0.05) ${val * 100}%, rgba(255,255,255,0.05) 100%)`,
                  }}
                />
              </div>
            );
          })}
        </div>

        {/* Info */}
        {snapshot && (
          <div className="w-full mt-3 pt-3 border-t border-white/5">
            <div className="flex items-center justify-between text-[10px] text-white/30">
              <span>基于第 {snapshot.chapter_number ?? '?'} 章分析</span>
              <span>{new Date(snapshot.computed_at).toLocaleDateString()} 计算</span>
            </div>
          </div>
        )}

        {hasChanges && (
          <div className="w-full mt-2 p-2 rounded-lg bg-cinema-gold/5 border border-cinema-gold/10">
            <p className="text-[10px] text-cinema-gold/70 text-center">
              调整后的维度将作为 AI 生成约束保存
            </p>
          </div>
        )}
      </div>
    </div>
  );
};

export default StyleDnaRadar;
