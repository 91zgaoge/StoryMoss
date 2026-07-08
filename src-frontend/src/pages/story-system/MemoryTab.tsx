import { useState, useEffect } from 'react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Brain, Zap } from 'lucide-react';
import { getMemoryItems, buildMemoryPack } from '@/services/tauri';
import type { MemoryItem } from '@/services/tauri';
import toast from 'react-hot-toast';

interface MemoryTabProps {
  storyId: string;
  selectedChapter: number;
}

export function MemoryTab({ storyId, selectedChapter }: MemoryTabProps) {
  const [memoryItems, setMemoryItems] = useState<MemoryItem[]>([]);

  const loadMemory = async () => {
    try {
      const data = await getMemoryItems(storyId);
      setMemoryItems(data);
    } catch {
      // silent fail
    }
  };

  useEffect(() => {
    loadMemory();
  }, [storyId]);

  const handleBuildMemoryPack = async () => {
    try {
      await buildMemoryPack(storyId, selectedChapter, 'write');
      toast.success('记忆包构建成功');
      loadMemory();
    } catch {
      toast.error('构建记忆包失败');
    }
  };

  return (
    <Card>
      <CardContent className="p-4">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold text-white flex items-center gap-2">
            <Brain className="w-5 h-5 text-purple-400" />
            记忆项 ({memoryItems.length})
          </h3>
          <Button size="sm" onClick={handleBuildMemoryPack}>
            <Zap className="w-4 h-4 mr-1" />
            构建记忆包
          </Button>
        </div>
        {memoryItems.length === 0 ? (
          <p className="text-gray-500 text-sm">暂无记忆项</p>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
            {memoryItems.slice(0, 30).map(item => (
              <div key={item.id} className="p-3 bg-cinema-800 rounded-lg">
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-xs px-2 py-0.5 rounded bg-cinema-700 text-gray-300">
                    {item.category}
                  </span>
                  <span className="text-xs text-gray-500">Ch{item.source_chapter || '?'}</span>
                </div>
                <p className="text-white text-sm">{item.subject || item.value || '(空)'}</p>
                <p className="text-gray-500 text-xs mt-1">
                  置信度: {(item.confidence * 100).toFixed(0)}%
                </p>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
