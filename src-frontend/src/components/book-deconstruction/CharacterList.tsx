import { User, Users } from 'lucide-react';
import type { ReferenceCharacter } from '@/types/book-deconstruction';

interface CharacterListProps {
  characters: ReferenceCharacter[];
}

export function CharacterList({ characters }: CharacterListProps) {
  const getRoleBadge = (role?: string) => {
    const map: Record<string, { label: string; color: string }> = {
      主角: { label: '主角', color: 'bg-cinema-gold/20 text-cinema-gold' },
      主人公: { label: '主角', color: 'bg-cinema-gold/20 text-cinema-gold' },
      反派: { label: '反派', color: 'bg-red-500/20 text-red-400' },
      配角: { label: '配角', color: 'bg-blue-500/20 text-blue-400' },
      龙套: { label: '龙套', color: 'bg-gray-500/20 text-gray-400' },
    };
    return map[role || ''] || { label: role || '未知', color: 'bg-gray-500/20 text-gray-400' };
  };

  const getImportanceWidth = (score?: number) => {
    return `${((score || 0) * 100).toFixed(0)}%`;
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 mb-4">
        <Users className="w-5 h-5 text-cinema-gold" />
        <h3 className="text-lg font-medium text-white">人物角色</h3>
        <span className="text-sm text-gray-500">({characters.length})</span>
      </div>

      {characters.length === 0 ? (
        <div className="text-center py-8 text-gray-500">暂无人物数据</div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {characters.map(char => {
            const badge = getRoleBadge(char.role_type);
            return (
              <div
                key={char.id}
                className="bg-cinema-900 border border-cinema-800 rounded-xl p-4 hover:border-cinema-700 transition-colors"
              >
                <div className="flex items-start gap-3">
                  <div className="w-10 h-10 rounded-full bg-cinema-800 flex items-center justify-center flex-shrink-0">
                    <User className="w-5 h-5 text-cinema-gold" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <h4 className="text-sm font-medium text-white">{char.name}</h4>
                      <span className={`text-xs px-2 py-0.5 rounded-full ${badge.color}`}>
                        {badge.label}
                      </span>
                    </div>
                    {char.personality && (
                      <p className="text-xs text-gray-400 mt-1 line-clamp-2">{char.personality}</p>
                    )}
                    {char.appearance && (
                      <p className="text-xs text-gray-600 mt-1 line-clamp-1">{char.appearance}</p>
                    )}
                    {/* 重要度条 */}
                    {char.importance_score !== undefined && (
                      <div className="mt-2">
                        <div className="h-1 bg-cinema-800 rounded-full overflow-hidden">
                          <div
                            className="h-full bg-cinema-gold rounded-full"
                            style={{ width: getImportanceWidth(char.importance_score) }}
                          />
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
