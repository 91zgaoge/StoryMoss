import { useState } from 'react';
import { useAppStore } from '@/stores/appStore';
import { useCharacters, useCreateCharacter, useDeleteCharacter } from '@/hooks/useCharacters';
import {
  useCharacterRelationships,
  useDeleteCharacterRelationship,
} from '@/hooks/useCharacterRelationships';
import { useWorldBuilding } from '@/hooks/useWorldBuilding';
import { useQueryClient } from '@tanstack/react-query';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { CharacterStatePanel } from '@/components/CharacterStatePanel';
import { CharacterEditModal } from '@/components/CharacterEditModal';
import { CharacterRelationshipForm } from '@/components/CharacterRelationshipForm';
import { generateCharacterProfiles } from '@/services/api/wizard';
import {
  Users,
  Plus,
  Trash2,
  Heart,
  UserX,
  Link2,
  Pencil,
  Star,
  Wand2,
  RefreshCw,
  X,
  Sparkles,
} from 'lucide-react';
import type {
  Character,
  CharacterRelationship,
  WorldBuilding,
  WorldBuildingOption,
  CharacterProfileOption,
} from '@/types';

type CharacterTab = 'info' | 'relationships';

function RelationshipCard({
  rel,
  characterId,
  storyId,
}: {
  rel: CharacterRelationship;
  characterId: string;
  storyId: string;
}) {
  const deleteRelationship = useDeleteCharacterRelationship();
  const isOutgoing = rel.source_character_id === characterId;

  const handleDelete = () => {
    if (confirm('确定要删除这个关系吗？')) {
      deleteRelationship.mutate({ relationshipId: rel.id, storyId });
    }
  };

  return (
    <div className="p-3 bg-cinema-800/50 rounded-lg border border-cinema-700">
      <div className="flex items-center justify-between gap-2 text-sm">
        <div className="flex items-center gap-2">
          <Link2 className="w-3.5 h-3.5 text-cinema-gold" />
          <span className="text-white font-medium">{isOutgoing ? '→' : '←'}</span>
          <span className="text-cinema-gold">{rel.relationship_type}</span>
          {rel.target_character_name && (
            <span className="text-gray-400">
              {isOutgoing ? '对' : '来自'} {rel.target_character_name}
            </span>
          )}
        </div>
        <button
          onClick={handleDelete}
          disabled={deleteRelationship.isPending}
          className="p-1.5 rounded-lg hover:bg-red-500/20 text-red-400 transition-colors disabled:opacity-50"
          title="删除关系"
          data-testid={`delete-relationship-${rel.id}`}
        >
          <Trash2 className="w-3.5 h-3.5" />
        </button>
      </div>
      {rel.description && (
        <p className="mt-1 text-xs text-gray-500 line-clamp-2">{rel.description}</p>
      )}
      {rel.dynamic && <p className="mt-1 text-xs text-gray-600 italic">动态: {rel.dynamic}</p>}
    </div>
  );
}

export function Characters() {
  const currentStory = useAppStore(s => s.currentStory);
  const queryClient = useQueryClient();
  const { data: characters = [] } = useCharacters(currentStory?.id || null);
  const { data: relationships = [] } = useCharacterRelationships(currentStory?.id || undefined);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [activeTab, setActiveTab] = useState<CharacterTab>('info');
  const [selectedCharacterId, setSelectedCharacterId] = useState<string | null>(null);
  const [editingCharacter, setEditingCharacter] = useState<Character | null>(null);
  const [relationshipFormOpen, setRelationshipFormOpen] = useState(false);
  const [relationshipFormDefaultCharacterId, setRelationshipFormDefaultCharacterId] = useState<
    string | null
  >(null);
  const [aiModalOpen, setAiModalOpen] = useState(false);
  const [aiGenerating, setAiGenerating] = useState(false);
  const [characterSets, setCharacterSets] = useState<CharacterProfileOption[][]>([]);
  const [selectedSetIndex, setSelectedSetIndex] = useState<number | null>(null);

  const { data: worldBuilding } = useWorldBuilding(currentStory?.id || null);
  const createCharacter = useCreateCharacter();
  const deleteCharacter = useDeleteCharacter();

  const handleCreate = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!currentStory) return;

    const form = e.currentTarget;
    const formData = new FormData(form);

    createCharacter.mutate(
      {
        story_id: currentStory.id,
        name: formData.get('name') as string,
        background: (formData.get('background') as string) || undefined,
        personality: (formData.get('personality') as string) || undefined,
        goals: (formData.get('goals') as string) || undefined,
        appearance: (formData.get('appearance') as string) || undefined,
        gender: (formData.get('gender') as string) || undefined,
        age: formData.get('age') ? Number(formData.get('age')) : undefined,
      },
      {
        onSuccess: () => {
          setIsModalOpen(false);
          form.reset();
        },
      }
    );
  };

  const handleDelete = (id: string) => {
    if (confirm('确定要删除这个角色吗？')) {
      deleteCharacter.mutate(id);
      if (selectedCharacterId === id) {
        setSelectedCharacterId(null);
      }
    }
  };

  const handleOpenRelationshipForm = (defaultCharacterId?: string) => {
    setRelationshipFormDefaultCharacterId(defaultCharacterId || null);
    setRelationshipFormOpen(true);
  };

  const toWorldBuildingOption = (wb: WorldBuilding): WorldBuildingOption => ({
    id: wb.id,
    concept: wb.concept,
    rules: wb.rules,
    history: wb.history,
    cultures: wb.cultures,
  });

  const handleGenerateCharacters = async () => {
    if (!worldBuilding) return;
    setAiGenerating(true);
    try {
      const option = toWorldBuildingOption(worldBuilding);
      const sets = await generateCharacterProfiles(option);
      setCharacterSets(sets);
      setSelectedSetIndex(sets.length > 0 ? 0 : null);
    } finally {
      setAiGenerating(false);
    }
  };

  const handleApplyCharacterSet = () => {
    if (selectedSetIndex === null || !currentStory) return;
    const set = characterSets[selectedSetIndex];
    set.forEach(profile => {
      createCharacter.mutate({
        story_id: currentStory.id,
        name: profile.name,
        personality: profile.personality,
        background: profile.background,
        goals: profile.goals,
      });
    });
    setAiModalOpen(false);
    setCharacterSets([]);
    setSelectedSetIndex(null);
  };

  const getCharacterRelationships = (charId: string) => {
    return relationships.filter(
      r => r.source_character_id === charId || r.target_character_id === charId
    );
  };

  if (!currentStory) {
    return (
      <div className="p-8 flex items-center justify-center h-full">
        <Card>
          <CardContent className="p-8 text-center">
            <Users className="w-12 h-12 text-gray-600 mx-auto mb-4" />
            <h2 className="font-display text-xl font-semibold text-white mb-2">先选择一个故事</h2>
            <p className="text-gray-400">在故事库中选择一个故事来管理角色</p>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="p-8 space-y-6 animate-fade-in">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="font-display text-3xl font-bold text-white">角色管理</h1>
          <p className="text-gray-400">
            {currentStory.title} - 共 {characters.length} 个角色
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="secondary" onClick={() => setAiModalOpen(true)}>
            <Wand2 className="w-4 h-4" />
            AI 扩展
          </Button>
          <Button variant="primary" onClick={() => setIsModalOpen(true)}>
            <Plus className="w-4 h-4" />
            添加角色
          </Button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 p-1 bg-cinema-800 rounded-lg w-fit">
        <button
          onClick={() => setActiveTab('info')}
          className={`px-4 py-1.5 rounded-md text-sm transition-colors ${
            activeTab === 'info' ? 'bg-cinema-700 text-white' : 'text-gray-400 hover:text-gray-200'
          }`}
        >
          资料
        </button>
        <button
          onClick={() => setActiveTab('relationships')}
          className={`px-4 py-1.5 rounded-md text-sm transition-colors ${
            activeTab === 'relationships'
              ? 'bg-cinema-700 text-white'
              : 'text-gray-400 hover:text-gray-200'
          }`}
        >
          关系
        </button>
      </div>

      {activeTab === 'info' ? (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {characters.map(char => (
            <Card key={char.id} hover className="group">
              <CardContent className="p-6">
                <div className="flex items-center gap-4">
                  <div className="w-14 h-14 rounded-full bg-cinema-velvet/20 flex items-center justify-center text-cinema-velvet font-display text-xl">
                    {char.name.charAt(0)}
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <h3 className="font-display text-lg font-semibold text-white truncate">
                        {char.name}
                      </h3>
                      {char.is_auto_generated && (
                        <span className="text-xs px-1.5 py-0.5 rounded bg-cinema-gold/20 text-cinema-gold flex items-center gap-1 shrink-0">
                          <Star className="w-3 h-3" />
                          创世
                        </span>
                      )}
                    </div>
                    {char.personality && (
                      <p className="text-sm text-gray-400 mt-1 line-clamp-1">{char.personality}</p>
                    )}
                  </div>
                  <div className="flex items-center gap-1">
                    <button
                      onClick={() => setEditingCharacter(char)}
                      className="p-2 rounded-lg opacity-0 group-hover:opacity-100 hover:bg-cinema-gold/20 text-cinema-gold transition-all"
                      title="编辑"
                    >
                      <Pencil className="w-4 h-4" />
                    </button>
                    <button
                      onClick={() => handleDelete(char.id)}
                      className="p-2 rounded-lg opacity-0 group-hover:opacity-100 hover:bg-red-500/20 text-red-400 transition-all"
                      title="删除"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </div>

                {/* Detail fields */}
                <div className="mt-4 space-y-2">
                  {char.appearance && (
                    <div className="flex items-start gap-2">
                      <UserX className="w-3.5 h-3.5 text-gray-500 mt-0.5 flex-shrink-0" />
                      <p className="text-sm text-gray-500 line-clamp-2">{char.appearance}</p>
                    </div>
                  )}
                  {char.goals && (
                    <div className="flex items-start gap-2">
                      <Heart className="w-3.5 h-3.5 text-gray-500 mt-0.5 flex-shrink-0" />
                      <p className="text-sm text-gray-500 line-clamp-2">{char.goals}</p>
                    </div>
                  )}
                  {char.background && (
                    <p className="text-sm text-gray-600 line-clamp-2">{char.background}</p>
                  )}
                  <div className="flex flex-wrap gap-2 pt-1">
                    {char.gender && (
                      <span className="text-xs px-2 py-0.5 rounded-full bg-cinema-800 text-gray-400">
                        {char.gender}
                      </span>
                    )}
                    {char.age != null && (
                      <span className="text-xs px-2 py-0.5 rounded-full bg-cinema-800 text-gray-400">
                        {char.age} 岁
                      </span>
                    )}
                  </div>
                </div>

                <CharacterStatePanel
                  character={char}
                  onUpdate={() => {
                    if (currentStory?.id) {
                      queryClient.invalidateQueries({ queryKey: ['characters', currentStory.id] });
                    }
                  }}
                />
              </CardContent>
            </Card>
          ))}

          {characters.length === 0 && (
            <div className="col-span-full text-center py-12">
              <Users className="w-16 h-16 text-gray-700 mx-auto mb-4" />
              <p className="text-gray-500">还没有角色，添加一个吧！</p>
            </div>
          )}
        </div>
      ) : (
        <div className="space-y-6">
          <div className="flex justify-end">
            <Button variant="primary" onClick={() => handleOpenRelationshipForm()}>
              <Plus className="w-4 h-4" />
              添加关系
            </Button>
          </div>

          {characters.map(char => {
            const charRels = getCharacterRelationships(char.id);
            return (
              <Card key={char.id}>
                <CardContent className="p-6">
                  <div className="flex items-center justify-between mb-4">
                    <div className="flex items-center gap-3">
                      <div className="w-10 h-10 rounded-full bg-cinema-velvet/20 flex items-center justify-center text-cinema-velvet font-display text-lg">
                        {char.name.charAt(0)}
                      </div>
                      <div className="flex items-center gap-2">
                        <h3 className="font-display text-lg font-semibold text-white">
                          {char.name}
                        </h3>
                        {char.is_auto_generated && (
                          <span className="text-xs px-1.5 py-0.5 rounded bg-cinema-gold/20 text-cinema-gold flex items-center gap-1">
                            <Star className="w-3 h-3" />
                            创世
                          </span>
                        )}
                      </div>
                      <span className="text-xs text-gray-500">{charRels.length} 个关系</span>
                    </div>
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => handleOpenRelationshipForm(char.id)}
                    >
                      <Plus className="w-3.5 h-3.5" />
                      添加关系
                    </Button>
                  </div>

                  {charRels.length > 0 ? (
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
                      {charRels.map(rel => (
                        <RelationshipCard
                          key={rel.id}
                          rel={rel}
                          characterId={char.id}
                          storyId={currentStory.id}
                        />
                      ))}
                    </div>
                  ) : (
                    <p className="text-sm text-gray-500">暂无关系数据</p>
                  )}
                </CardContent>
              </Card>
            );
          })}

          {characters.length === 0 && (
            <div className="text-center py-12">
              <Users className="w-16 h-16 text-gray-700 mx-auto mb-4" />
              <p className="text-gray-500">还没有角色，添加一个吧！</p>
            </div>
          )}
        </div>
      )}

      {/* Create Modal */}
      {isModalOpen && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <Card className="w-full max-w-lg mx-4">
            <CardContent className="p-6">
              <h2 className="font-display text-xl font-bold text-white mb-4">添加角色</h2>

              <form onSubmit={handleCreate} className="space-y-4">
                <div>
                  <label className="block text-sm text-gray-400 mb-1">角色名称 *</label>
                  <input
                    name="name"
                    required
                    className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                    placeholder="输入角色名称"
                  />
                </div>

                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">性别</label>
                    <input
                      name="gender"
                      className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                      placeholder="性别"
                    />
                  </div>
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">年龄</label>
                    <input
                      name="age"
                      type="number"
                      className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                      placeholder="年龄"
                    />
                  </div>
                </div>

                <div>
                  <label className="block text-sm text-gray-400 mb-1">性格</label>
                  <textarea
                    name="personality"
                    rows={2}
                    className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                    placeholder="角色的性格特点..."
                  />
                </div>

                <div>
                  <label className="block text-sm text-gray-400 mb-1">外貌</label>
                  <textarea
                    name="appearance"
                    rows={2}
                    className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                    placeholder="角色的外貌描述..."
                  />
                </div>

                <div>
                  <label className="block text-sm text-gray-400 mb-1">目标</label>
                  <textarea
                    name="goals"
                    rows={2}
                    className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                    placeholder="角色的目标与动机..."
                  />
                </div>

                <div>
                  <label className="block text-sm text-gray-400 mb-1">背景故事</label>
                  <textarea
                    name="background"
                    rows={3}
                    className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                    placeholder="角色的背景故事..."
                  />
                </div>

                <div className="flex gap-3 pt-4">
                  <Button type="button" variant="ghost" onClick={() => setIsModalOpen(false)}>
                    取消
                  </Button>
                  <Button type="submit" variant="primary" isLoading={createCharacter.isPending}>
                    创建
                  </Button>
                </div>
              </form>
            </CardContent>
          </Card>
        </div>
      )}

      <CharacterEditModal
        character={editingCharacter}
        isOpen={!!editingCharacter}
        onClose={() => setEditingCharacter(null)}
      />

      <CharacterRelationshipForm
        storyId={currentStory.id}
        characters={characters}
        defaultCharacterId={relationshipFormDefaultCharacterId}
        isOpen={relationshipFormOpen}
        onClose={() => {
          setRelationshipFormOpen(false);
          setRelationshipFormDefaultCharacterId(null);
        }}
      />

      {/* AI Expansion Modal */}
      {aiModalOpen && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
          <Card className="w-full max-w-3xl max-h-[90vh] overflow-y-auto">
            <CardContent className="p-6 space-y-5">
              <div className="flex items-center justify-between">
                <h2 className="font-display text-xl font-bold text-white flex items-center gap-2">
                  <Wand2 className="w-5 h-5 text-cinema-gold" />
                  AI 扩展角色
                </h2>
                <button
                  onClick={() => {
                    setAiModalOpen(false);
                    setCharacterSets([]);
                    setSelectedSetIndex(null);
                  }}
                  className="p-1.5 rounded-lg hover:bg-cinema-700 text-gray-400 hover:text-white transition-colors"
                >
                  <X className="w-5 h-5" />
                </button>
              </div>

              {!worldBuilding ? (
                <div className="text-center py-8 text-gray-400">
                  <p>请先在世界构建页初始化世界观，AI 需要基于世界观生成角色。</p>
                </div>
              ) : (
                <>
                  <div className="flex items-center justify-between">
                    <p className="text-sm text-gray-400">
                      基于当前世界观生成若干角色组合，选择一套加入故事。
                    </p>
                    <Button
                      variant="secondary"
                      onClick={handleGenerateCharacters}
                      isLoading={aiGenerating}
                      disabled={aiGenerating}
                    >
                      {aiGenerating ? (
                        <RefreshCw className="w-4 h-4 animate-spin" />
                      ) : (
                        <Sparkles className="w-4 h-4" />
                      )}
                      生成角色组合
                    </Button>
                  </div>

                  {characterSets.length > 0 && (
                    <div className="space-y-4">
                      <p className="text-sm text-gray-400">选择一组角色：</p>
                      <div className="grid gap-3">
                        {characterSets.map((set, setIdx) => (
                          <div
                            key={setIdx}
                            onClick={() => setSelectedSetIndex(setIdx)}
                            className={`p-4 rounded-xl border cursor-pointer transition-all ${
                              selectedSetIndex === setIdx
                                ? 'border-cinema-gold bg-cinema-gold/10'
                                : 'border-cinema-700 bg-cinema-800/50 hover:border-cinema-gold/40'
                            }`}
                          >
                            <div className="flex items-center justify-between mb-2">
                              <span className="text-sm font-medium text-white">
                                组合 {setIdx + 1}
                              </span>
                              <span className="text-xs text-gray-500">{set.length} 个角色</span>
                            </div>
                            <div className="flex flex-wrap gap-2">
                              {set.map(char => (
                                <span
                                  key={char.name}
                                  className="text-xs px-2 py-1 rounded-full bg-cinema-900/80 text-gray-300 border border-cinema-700"
                                >
                                  {char.name}
                                </span>
                              ))}
                            </div>
                          </div>
                        ))}
                      </div>
                      <Button
                        variant="primary"
                        onClick={handleApplyCharacterSet}
                        disabled={selectedSetIndex === null || createCharacter.isPending}
                        className="w-full"
                      >
                        添加选中的角色组
                      </Button>
                    </div>
                  )}
                </>
              )}
            </CardContent>
          </Card>
        </div>
      )}
    </div>
  );
}
