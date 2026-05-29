import { ChevronDown, ChevronRight, BookMarked } from 'lucide-react';
import { useState } from 'react';
import type { ReferenceScene } from '@/types/book-deconstruction';

interface ChapterOutlineProps {
  scenes: ReferenceScene[];
}

export function ChapterOutline({ scenes }: ChapterOutlineProps) {
  const [expanded, setExpanded] = useState<Set<number>>(new Set());

  const toggleExpand = (index: number) => {
    const newSet = new Set(expanded);
    if (newSet.has(index)) {
      newSet.delete(index);
    } else {
      newSet.add(index);
    }
    setExpanded(newSet);
  };

  const parseCharacters = (chars?: string) => {
    if (!chars) return [];
    try {
      return JSON.parse(chars) as string[];
    } catch {
      return [];
    }
  };

  const parseEvents = (events?: string) => {
    if (!events) return [];
    try {
      return JSON.parse(events) as string[];
    } catch {
      return [];
    }
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 mb-4">
        <BookMarked className="w-5 h-5 text-cinema-gold" />
        <h3 className="text-lg font-medium text-white">章节大纲</h3>
        <span className="text-sm text-gray-500">({scenes.length} 章)</span>
      </div>

      {scenes.length === 0 ? (
        <div className="text-center py-8 text-gray-500">暂无章节数据</div>
      ) : (
        <div className="space-y-2">
          {scenes.map((scene, index) => {
            const isExpanded = expanded.has(index);
            const characters = parseCharacters(scene.characters_present);
            const events = parseEvents(scene.key_events);

            return (
              <div
                key={scene.id}
                className="bg-cinema-900 border border-cinema-800 rounded-xl overflow-hidden"
              >
                <button
                  onClick={() => toggleExpand(index)}
                  className="w-full flex items-center gap-3 p-4 hover:bg-cinema-800/50 transition-colors text-left"
                >
                  {isExpanded ? (
                    <ChevronDown className="w-4 h-4 text-gray-500 flex-shrink-0" />
                  ) : (
                    <ChevronRight className="w-4 h-4 text-gray-500 flex-shrink-0" />
                  )}
                  <span className="text-xs text-gray-500 w-12 flex-shrink-0">
                    第{scene.sequence_number}章
                  </span>
                  <span className="text-sm font-medium text-white flex-1 truncate">
                    {scene.title || '未命名章节'}
                  </span>
                  {scene.emotional_tone && (
                    <span className="text-xs text-gray-600 flex-shrink-0">
                      {scene.emotional_tone}
                    </span>
                  )}
                </button>

                {isExpanded && (
                  <div className="px-4 pb-4 pl-12">
                    <p className="text-sm text-gray-400 mb-3">{scene.summary || '暂无概要'}</p>

                    {characters.length > 0 && (
                      <div className="flex flex-wrap gap-1.5 mb-2">
                        <span className="text-xs text-gray-600">出场:</span>
                        {characters.map(name => (
                          <span
                            key={name}
                            className="text-xs px-2 py-0.5 rounded bg-cinema-800 text-gray-400"
                          >
                            {name}
                          </span>
                        ))}
                      </div>
                    )}

                    {events.length > 0 && (
                      <div className="space-y-1">
                        <span className="text-xs text-gray-600">关键事件:</span>
                        {events.map((event, i) => (
                          <div key={i} className="text-xs text-gray-500 pl-2">
                            • {event}
                          </div>
                        ))}
                      </div>
                    )}

                    {scene.conflict_type && (
                      <div className="mt-2 text-xs text-gray-600">冲突: {scene.conflict_type}</div>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
