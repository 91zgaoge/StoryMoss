import { useState, useEffect, useCallback } from 'react';
import { X, Plus, AlertTriangle } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import type { StyleBlendConfig, BlendComponent, BlendRole, StyleDNA } from '@/types/index';

interface StyleBlendPanelProps {
  storyId: string;
  availableDnas: StyleDNA[];
  initialBlend?: StyleBlendConfig;
  onSave: (blend: StyleBlendConfig) => void;
  onCancel: () => void;
}

function computeRole(weight: number): BlendRole {
  if (weight >= 0.5) return 'dominant';
  if (weight >= 0.2) return 'secondary';
  return 'tertiary';
}

export function StyleBlendPanel({
  availableDnas,
  initialBlend,
  onSave,
  onCancel,
}: StyleBlendPanelProps) {
  const [configName, setConfigName] = useState(initialBlend?.name || '默认混合');
  const [components, setComponents] = useState<BlendComponent[]>(initialBlend?.components || []);
  const [driftEnabled, setDriftEnabled] = useState(initialBlend?.drift_check_enabled ?? true);
  const [errors, setErrors] = useState<string[]>([]);

  const normalizeWeights = useCallback((comps: BlendComponent[]): BlendComponent[] => {
    const total = comps.reduce((sum, c) => sum + c.weight, 0);
    if (total <= 0) return comps;
    return comps.map(c => ({
      ...c,
      weight: parseFloat((c.weight / total).toFixed(3)),
      role: computeRole(c.weight / total),
    }));
  }, []);

  const validate = useCallback((): string[] => {
    const errs: string[] = [];
    if (components.length === 0) errs.push('混合配置不能为空');
    if (components.length > 5) errs.push('最多支持 5 个风格混合');
    const total = components.reduce((sum, c) => sum + c.weight, 0);
    if (Math.abs(total - 1.0) > 0.01)
      errs.push(`权重总和必须为 100%，当前为 ${(total * 100).toFixed(0)}%`);
    const dominantCount = components.filter(c => c.role === 'dominant').length;
    if (dominantCount === 0) errs.push('必须有一个主导风格（权重 >= 50%）');
    return errs;
  }, [components]);

  useEffect(() => {
    setErrors(validate());
  }, [components, validate]);

  const handleWeightChange = (index: number, newWeightPercent: number) => {
    const newComps = [...components];
    newComps[index] = {
      ...newComps[index],
      weight: newWeightPercent / 100,
      role: computeRole(newWeightPercent / 100),
    };
    setComponents(normalizeWeights(newComps));
  };

  const handleAddStyle = (dna: StyleDNA) => {
    if (components.length >= 5) return;
    if (components.some(c => c.dna_id === dna.id)) return;
    const newComps = [
      ...components,
      {
        dna_id: dna.id,
        dna_name: dna.name,
        weight: 0,
        role: 'tertiary' as BlendRole,
      },
    ];
    // 平均分配权重
    const avg = 1.0 / newComps.length;
    setComponents(normalizeWeights(newComps.map(c => ({ ...c, weight: avg }))));
  };

  const handleRemove = (index: number) => {
    const newComps = components.filter((_, i) => i !== index);
    if (newComps.length > 0) {
      const avg = 1.0 / newComps.length;
      setComponents(normalizeWeights(newComps.map(c => ({ ...c, weight: avg }))));
    } else {
      setComponents([]);
    }
  };

  const handleSave = () => {
    const errs = validate();
    if (errs.length > 0) {
      setErrors(errs);
      return;
    }
    onSave({
      name: configName,
      components,
      drift_check_enabled: driftEnabled,
    });
  };

  const unusedDnas = availableDnas.filter(d => !components.some(c => c.dna_id === d.id));

  return (
    <div className="space-y-5">
      {/* 配置名称 */}
      <div>
        <label className="block text-sm text-gray-400 mb-1">配置名称</label>
        <input
          type="text"
          value={configName}
          onChange={e => setConfigName(e.target.value)}
          className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
        />
      </div>

      {/* 已选风格列表 */}
      {components.length > 0 && (
        <div className="space-y-3">
          {components.map((comp, idx) => (
            <div
              key={comp.dna_id}
              className="p-3 rounded-lg bg-cinema-800 border border-cinema-700"
            >
              <div className="flex items-center justify-between mb-2">
                <div className="flex items-center gap-2">
                  <span className="font-medium text-white">{comp.dna_name}</span>
                  <span
                    className={`text-[10px] px-1.5 py-0.5 rounded ${
                      comp.role === 'dominant'
                        ? 'bg-cinema-gold/20 text-cinema-gold'
                        : 'bg-cinema-700 text-gray-400'
                    }`}
                  >
                    {comp.role === 'dominant'
                      ? '主导'
                      : comp.role === 'secondary'
                        ? '辅助'
                        : '点缀'}
                  </span>
                </div>
                <button
                  onClick={() => handleRemove(idx)}
                  className="text-gray-500 hover:text-red-400 transition-colors"
                >
                  <X className="w-4 h-4" />
                </button>
              </div>
              <div className="flex items-center gap-3">
                <input
                  type="range"
                  min="0"
                  max="100"
                  value={Math.round(comp.weight * 100)}
                  onChange={e => handleWeightChange(idx, Number(e.target.value))}
                  className="flex-1 h-1.5 bg-cinema-700 rounded-lg appearance-none cursor-pointer accent-cinema-gold"
                />
                <span className="text-sm text-cinema-gold w-12 text-right">
                  {Math.round(comp.weight * 100)}%
                </span>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* 添加风格 */}
      {unusedDnas.length > 0 && (
        <div>
          <label className="block text-sm text-gray-400 mb-2">添加风格（最多5个）</label>
          <div className="flex flex-wrap gap-2">
            {unusedDnas.map(dna => (
              <button
                key={dna.id}
                onClick={() => handleAddStyle(dna)}
                className="px-3 py-1.5 rounded-lg bg-cinema-800 border border-cinema-700 text-sm text-gray-300 hover:bg-cinema-700 hover:text-white transition-colors flex items-center gap-1"
              >
                <Plus className="w-3 h-3" />
                {dna.name}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* 防漂移自检 */}
      <label className="flex items-center gap-2 cursor-pointer">
        <input
          type="checkbox"
          checked={driftEnabled}
          onChange={e => setDriftEnabled(e.target.checked)}
          className="w-4 h-4 rounded border-cinema-700 bg-cinema-800 text-cinema-gold focus:ring-cinema-gold"
        />
        <span className="text-sm text-gray-300">启用防漂移自检</span>
      </label>

      {/* 验证错误 */}
      {errors.length > 0 && (
        <div className="p-3 rounded-lg bg-red-900/20 border border-red-800/50 space-y-1">
          {errors.map((err, i) => (
            <div key={i} className="flex items-center gap-1.5 text-xs text-red-400">
              <AlertTriangle className="w-3 h-3" />
              {err}
            </div>
          ))}
        </div>
      )}

      {/* 按钮 */}
      <div className="flex justify-end gap-2 pt-2">
        <Button variant="ghost" size="sm" onClick={onCancel}>
          取消
        </Button>
        <Button variant="primary" size="sm" onClick={handleSave} disabled={errors.length > 0}>
          保存配置
        </Button>
      </div>
    </div>
  );
}
