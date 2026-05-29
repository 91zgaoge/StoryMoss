import { useState } from 'react';
import {
  MapPin,
  Zap,
  Heart,
  Brain,
  Backpack,
  Clock,
  Activity,
  ChevronDown,
  ChevronUp,
  Save,
  X,
} from 'lucide-react';
import { cn } from '@/utils/cn';
import { updateCharacterState } from '@/services/tauri';
import type { Character, CharacterState } from '@/types';
import toast from 'react-hot-toast';

interface CharacterStatePanelProps {
  character: Character;
  onUpdate?: () => void;
}

const stateFields: {
  key: keyof CharacterState;
  label: string;
  icon: React.ElementType;
  placeholder: string;
}[] = [
  { key: 'location', label: '位置', icon: MapPin, placeholder: '例如：长安城、飞船甲板' },
  { key: 'power_level', label: '实力', icon: Zap, placeholder: '例如：筑基期、S级异能者' },
  { key: 'physical_state', label: '身体', icon: Heart, placeholder: '例如：轻伤、疲惫、健康' },
  { key: 'mental_state', label: '心理', icon: Brain, placeholder: '例如：焦虑、坚定、迷茫' },
  { key: 'key_items', label: '持有物品', icon: Backpack, placeholder: '例如：玉佩、激光枪、密信' },
  {
    key: 'recent_events',
    label: '近期事件',
    icon: Clock,
    placeholder: '例如：与主角决裂、获得传承',
  },
];

export function CharacterStatePanel({ character, onUpdate }: CharacterStatePanelProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [formData, setFormData] = useState<CharacterState>({
    location: character.cs_location || '',
    power_level: character.cs_power_level || '',
    physical_state: character.cs_physical_state || '',
    mental_state: character.cs_mental_state || '',
    key_items: character.cs_key_items || '',
    recent_events: character.cs_recent_events || '',
    updated_at_chapter: character.cs_updated_at_chapter,
  });

  const hasAnyState = stateFields.some(f => character[`cs_${f.key}` as keyof Character]);

  const handleSave = async () => {
    setIsSaving(true);
    try {
      const state: CharacterState = {
        location: formData.location || undefined,
        power_level: formData.power_level || undefined,
        physical_state: formData.physical_state || undefined,
        mental_state: formData.mental_state || undefined,
        key_items: formData.key_items || undefined,
        recent_events: formData.recent_events || undefined,
        updated_at_chapter: formData.updated_at_chapter,
      };
      await updateCharacterState(character.id, state);
      toast.success('角色状态已更新');
      setIsEditing(false);
      onUpdate?.();
    } catch (e: any) {
      toast.error('更新失败: ' + (e.message || String(e)));
    } finally {
      setIsSaving(false);
    }
  };

  const handleCancel = () => {
    setFormData({
      location: character.cs_location || '',
      power_level: character.cs_power_level || '',
      physical_state: character.cs_physical_state || '',
      mental_state: character.cs_mental_state || '',
      key_items: character.cs_key_items || '',
      recent_events: character.cs_recent_events || '',
      updated_at_chapter: character.cs_updated_at_chapter,
    });
    setIsEditing(false);
  };

  return (
    <div className="mt-3 border-t border-cinema-700/50 pt-3">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center justify-between text-xs text-cinema-gold/80 hover:text-cinema-gold transition-colors"
      >
        <span className="flex items-center gap-1.5">
          <Activity className="w-3.5 h-3.5" />
          动态状态
          {hasAnyState && <span className="w-1.5 h-1.5 rounded-full bg-green-400" />}
        </span>
        {isExpanded ? (
          <ChevronUp className="w-3.5 h-3.5" />
        ) : (
          <ChevronDown className="w-3.5 h-3.5" />
        )}
      </button>

      {isExpanded && (
        <div className="mt-2 space-y-2">
          {isEditing ? (
            <div className="space-y-2">
              {stateFields.map(({ key, label, icon: Icon, placeholder }) => (
                <div key={key} className="flex items-start gap-2">
                  <Icon className="w-3.5 h-3.5 text-gray-500 mt-1.5 flex-shrink-0" />
                  <div className="flex-1">
                    <label className="text-[10px] text-gray-500 uppercase tracking-wider">
                      {label}
                    </label>
                    <input
                      type="text"
                      value={(formData[key] as string) || ''}
                      onChange={e => setFormData(prev => ({ ...prev, [key]: e.target.value }))}
                      placeholder={placeholder}
                      className="w-full mt-0.5 px-2 py-1 text-xs bg-cinema-800/50 border border-cinema-700 rounded text-white placeholder:text-gray-600 focus:outline-none focus:border-cinema-gold/50"
                    />
                  </div>
                </div>
              ))}
              <div className="flex items-center justify-end gap-2 pt-1">
                <button
                  onClick={handleCancel}
                  className="flex items-center gap-1 px-2 py-1 text-[11px] text-gray-400 hover:text-white transition-colors"
                >
                  <X className="w-3 h-3" />
                  取消
                </button>
                <button
                  onClick={handleSave}
                  disabled={isSaving}
                  className="flex items-center gap-1 px-2 py-1 text-[11px] bg-cinema-gold/20 text-cinema-gold rounded hover:bg-cinema-gold/30 transition-colors disabled:opacity-50"
                >
                  <Save className="w-3 h-3" />
                  {isSaving ? '保存中...' : '保存'}
                </button>
              </div>
            </div>
          ) : (
            <div className="space-y-1.5">
              {stateFields.map(({ key, label, icon: Icon }) => {
                const value = character[`cs_${key}` as keyof Character] as string | undefined;
                if (!value) return null;
                return (
                  <div key={key} className="flex items-center gap-2 text-xs">
                    <Icon className="w-3.5 h-3.5 text-gray-500 flex-shrink-0" />
                    <span className="text-gray-500 min-w-[4em]">{label}</span>
                    <span className="text-white/80 truncate">{value}</span>
                  </div>
                );
              })}
              {character.cs_updated_at_chapter != null && (
                <div className="flex items-center gap-2 text-[11px] text-gray-600">
                  <Activity className="w-3 h-3" />
                  <span>更新于第 {character.cs_updated_at_chapter} 章</span>
                </div>
              )}
              {!hasAnyState && <p className="text-[11px] text-gray-600 italic">暂无动态状态记录</p>}
              <button
                onClick={() => setIsEditing(true)}
                className="w-full mt-1 py-1 text-[11px] text-cinema-gold/60 hover:text-cinema-gold border border-dashed border-cinema-700 hover:border-cinema-gold/30 rounded transition-colors"
              >
                {hasAnyState ? '编辑状态' : '添加状态'}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
