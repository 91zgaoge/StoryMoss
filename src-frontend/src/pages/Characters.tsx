import { useState, useEffect } from 'react';
import { useAppStore } from '@/stores/appStore';
import { useCharacters, useCreateCharacter, useDeleteCharacter } from '@/hooks/useCharacters';
import { useCharacterRelationships } from '@/hooks/useCharacterRelationships';
import { useQueryClient } from '@tanstack/react-query';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Users, Plus, Trash2, Heart, UserX, Link2 } from 'lucide-react';
import type { CharacterRelationship } from '@/types/index';

type CharacterTab = 'info' | 'relationships';

function RelationshipCard({ rel, characterId }: { rel: CharacterRelationship; characterId: string }) {
  const isOutgoing = rel.source_character_id === characterId;
  return (
    <div className="p-3 bg-cinema-800/50 rounded-lg border border-cinema-700">
      <div className="flex items-center gap-2 text-sm">
        <Link2 className="w-3.5 h-3.5 text-cinema-gold" />
        <span className="text-white font-medium">
          {isOutgoing ? '→' : '←'}
        </span>
        <span className="text-cinema-gold">{rel.relationship_type}</span>
        {rel.target_character_name && (
          <span className="text-gray-400">
            {isOutgoing ? '对' : '来自'} {rel.target_character_name}
          </span>
        )}
      </div>
      {rel.description && (
        <p className="mt-1 text-xs text-gray-500 line-clamp-2">{rel.description}</p>
      )}
      {rel.dynamic && (
        <p className="mt-1 text-xs text-gray-600 italic">动态: {rel.dynamic}</p>
      )}
    </div>
  );
}

export function Characters() {
  const currentStory = useAppStore((s) => s.currentStory);
  const queryClient = useQueryClient();
  const { data: characters = [] } = useCharacters(currentStory?.id || null);
  const { data: relationships = [] } = useCharacterRelationships(currentStory?.id || undefined);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [activeTab, setActiveTab] = useState<CharacterTab>('info');
  const [selectedCharacterId, setSelectedCharacterId] = useState<string | null>(null);

  // v5.0.0 修复：监听数据刷新事件
  useEffect(() => {
    const handleRefresh = () => {
      if (currentStory?.id) {
        queryClient.invalidateQueries({ queryKey: ['characters', currentStory.id] });
        queryClient.invalidateQueries({ queryKey: ['character-relationships', currentStory.id] });
      }
    };
    window.addEventListener('backstage-data-refreshed', handleRefresh);
    return () => window.removeEventListener('backstage-data-refreshed', handleRefresh);
  }, [currentStory?.id, queryClient]);

  const createCharacter = useCreateCharacter();
  const deleteCharacter = useDeleteCharacter();

  const handleCreate = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!currentStory) return;

    const form = e.currentTarget;
    const formData = new FormData(form);

    createCharacter.mutate({
      story_id: currentStory.id,
      name: formData.get('name') as string,
      background: formData.get('background') as string || undefined,
      personality: formData.get('personality') as string || undefined,
      goals: formData.get('goals') as string || undefined,
      appearance: formData.get('appearance') as string || undefined,
      gender: formData.get('gender') as string || undefined,
      age: formData.get('age') ? Number(formData.get('age')) : undefined,
    }, {
      onSuccess: () => {
        setIsModalOpen(false);
        form.reset();
      },
    });
  };

  const handleDelete = (id: string) => {
    if (confirm('确定要删除这个角色吗？')) {
      deleteCharacter.mutate(id);
      if (selectedCharacterId === id) {
        setSelectedCharacterId(null);
      }
    }
  };

  const getCharacterRelationships = (charId: string) => {
    return relationships.filter(
      (r) => r.source_character_id === charId || r.target_character_id === charId
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
          <p className="text-gray-400">{currentStory.title} - 共 {characters.length} 个角色</p>
        </div>
        <Button variant="primary" onClick={() => setIsModalOpen(true)}>
          <Plus className="w-4 h-4" />
          添加角色
        </Button>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 p-1 bg-cinema-800 rounded-lg w-fit">
        <button
          onClick={() => setActiveTab('info')}
          className={`px-4 py-1.5 rounded-md text-sm transition-colors ${
            activeTab === 'info'
              ? 'bg-cinema-700 text-white'
              : 'text-gray-400 hover:text-gray-200'
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
          {characters.map((char) => (
            <Card key={char.id} hover className="group">
              <CardContent className="p-6">
                <div className="flex items-center gap-4">
                  <div className="w-14 h-14 rounded-full bg-cinema-velvet/20 flex items-center justify-center text-cinema-velvet font-display text-xl">
                    {char.name.charAt(0)}
                  </div>
                  <div className="flex-1 min-w-0">
                    <h3 className="font-display text-lg font-semibold text-white truncate">{char.name}</h3>
                    {char.personality && (
                      <p className="text-sm text-gray-400 mt-1 line-clamp-1">{char.personality}</p>
                    )}
                  </div>
                  <button
                    onClick={() => handleDelete(char.id)}
                    className="p-2 rounded-lg opacity-0 group-hover:opacity-100 hover:bg-red-500/20 text-red-400 transition-all"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
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
          {characters.map((char) => {
            const charRels = getCharacterRelationships(char.id);
            return (
              <Card key={char.id}>
                <CardContent className="p-6">
                  <div className="flex items-center gap-3 mb-4">
                    <div className="w-10 h-10 rounded-full bg-cinema-velvet/20 flex items-center justify-center text-cinema-velvet font-display text-lg">
                      {char.name.charAt(0)}
                    </div>
                    <h3 className="font-display text-lg font-semibold text-white">{char.name}</h3>
                    <span className="text-xs text-gray-500">
                      {charRels.length} 个关系
                    </span>
                  </div>

                  {charRels.length > 0 ? (
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
                      {charRels.map((rel) => (
                        <RelationshipCard key={rel.id} rel={rel} characterId={char.id} />
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
                  <Button
                    type="submit"
                    variant="primary"
                    isLoading={createCharacter.isPending}
                  >
                    创建
                  </Button>
                </div>
              </form>
            </CardContent>
          </Card>
        </div>
      )}
    </div>
  );
}
