import { useState } from 'react';
import { X, Link2, Plus } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useCreateCharacterRelationship } from '@/hooks/useCharacterRelationships';
import type { Character } from '@/types/index';

/** 情感纽带可选项（与后端情感属性体系对齐，支持留空） */
const EMOTIONAL_BOND_OPTIONS = [
  '信任',
  '崇拜',
  '爱慕',
  '依赖',
  '憎恨',
  '恐惧',
  '欺骗',
  '嫉妒',
  '愧疚',
  '冷漠',
];

interface CharacterRelationshipFormProps {
  storyId: string;
  characters: Character[];
  defaultCharacterId?: string | null;
  isOpen: boolean;
  onClose: () => void;
}

export function CharacterRelationshipForm({
  storyId,
  characters,
  defaultCharacterId,
  isOpen,
  onClose,
}: CharacterRelationshipFormProps) {
  const createRelationship = useCreateCharacterRelationship();
  const [characterAId, setCharacterAId] = useState(defaultCharacterId || '');
  const [characterBId, setCharacterBId] = useState('');
  const [relationshipType, setRelationshipType] = useState('');
  const [description, setDescription] = useState('');
  const [emotionalBond, setEmotionalBond] = useState('');
  const [emotionalIntensity, setEmotionalIntensity] = useState(0.5);
  const [reverseEmotionalBond, setReverseEmotionalBond] = useState('');
  const [reverseEmotionalIntensity, setReverseEmotionalIntensity] = useState(0.5);

  if (!isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!characterAId || !characterBId || !relationshipType.trim()) return;

    createRelationship.mutate(
      {
        story_id: storyId,
        source_character_id: characterAId,
        target_character_id: characterBId,
        relationship_type: relationshipType.trim(),
        description: description.trim() || undefined,
        emotional_bond: emotionalBond || undefined,
        emotional_intensity: emotionalBond ? emotionalIntensity : undefined,
        reverse_emotional_bond: reverseEmotionalBond || undefined,
        reverse_emotional_intensity: reverseEmotionalBond ? reverseEmotionalIntensity : undefined,
      },
      {
        onSuccess: () => {
          setCharacterBId('');
          setRelationshipType('');
          setDescription('');
          setEmotionalBond('');
          setEmotionalIntensity(0.5);
          setReverseEmotionalBond('');
          setReverseEmotionalIntensity(0.5);
          onClose();
        },
      }
    );
  };

  return (
    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 animate-fade-in">
      <Card className="w-full max-w-md mx-4 animate-slide-up">
        <CardContent className="p-6">
          <div className="flex items-center gap-3 mb-6">
            <div className="p-2 rounded-xl bg-cinema-gold/10">
              <Link2 className="w-5 h-5 text-cinema-gold" />
            </div>
            <div>
              <h2 className="font-display text-xl font-bold text-white">添加关系</h2>
            </div>
            <button
              onClick={onClose}
              className="ml-auto p-2 rounded-lg text-gray-400 hover:text-white hover:bg-cinema-800 transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <label className="block text-sm text-gray-400 mb-1">角色 A *</label>
              <select
                value={characterAId}
                onChange={e => setCharacterAId(e.target.value)}
                required
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
              >
                <option value="">选择角色</option>
                {characters.map(char => (
                  <option key={char.id} value={char.id}>
                    {char.name}
                  </option>
                ))}
              </select>
            </div>

            <div>
              <label className="block text-sm text-gray-400 mb-1">角色 B *</label>
              <select
                value={characterBId}
                onChange={e => setCharacterBId(e.target.value)}
                required
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
              >
                <option value="">选择角色</option>
                {characters.map(char => (
                  <option key={char.id} value={char.id}>
                    {char.name}
                  </option>
                ))}
              </select>
            </div>

            <div>
              <label className="block text-sm text-gray-400 mb-1">关系类型 *</label>
              <input
                value={relationshipType}
                onChange={e => setRelationshipType(e.target.value)}
                required
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                placeholder="例如：朋友、敌人、恋人"
              />
            </div>

            <div>
              <label className="block text-sm text-gray-400 mb-1">关系描述</label>
              <textarea
                value={description}
                onChange={e => setDescription(e.target.value)}
                rows={3}
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                placeholder="描述这两个角色之间的关系..."
              />
            </div>

            <div>
              <label className="block text-sm text-gray-400 mb-1">A对B的情感</label>
              <select
                value={emotionalBond}
                onChange={e => setEmotionalBond(e.target.value)}
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
              >
                <option value="">无</option>
                {EMOTIONAL_BOND_OPTIONS.map(bond => (
                  <option key={bond} value={bond}>
                    {bond}
                  </option>
                ))}
              </select>
              {emotionalBond && (
                <div className="mt-2 flex items-center gap-3">
                  <input
                    type="range"
                    min={0}
                    max={1}
                    step={0.1}
                    value={emotionalIntensity}
                    onChange={e => setEmotionalIntensity(Number(e.target.value))}
                    className="flex-1 accent-cinema-gold"
                  />
                  <span className="text-sm text-gray-400 w-8 text-right">
                    {emotionalIntensity.toFixed(1)}
                  </span>
                </div>
              )}
            </div>

            <div>
              <label className="block text-sm text-gray-400 mb-1">B对A的情感</label>
              <select
                value={reverseEmotionalBond}
                onChange={e => setReverseEmotionalBond(e.target.value)}
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
              >
                <option value="">无</option>
                {EMOTIONAL_BOND_OPTIONS.map(bond => (
                  <option key={bond} value={bond}>
                    {bond}
                  </option>
                ))}
              </select>
              {reverseEmotionalBond && (
                <div className="mt-2 flex items-center gap-3">
                  <input
                    type="range"
                    min={0}
                    max={1}
                    step={0.1}
                    value={reverseEmotionalIntensity}
                    onChange={e => setReverseEmotionalIntensity(Number(e.target.value))}
                    className="flex-1 accent-cinema-gold"
                  />
                  <span className="text-sm text-gray-400 w-8 text-right">
                    {reverseEmotionalIntensity.toFixed(1)}
                  </span>
                </div>
              )}
            </div>

            <div className="flex gap-3 pt-4 border-t border-cinema-700">
              <Button type="button" variant="ghost" onClick={onClose}>
                取消
              </Button>
              <Button
                type="submit"
                variant="primary"
                isLoading={createRelationship.isPending}
                className="flex-1"
              >
                <Plus className="w-4 h-4" />
                添加
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
