import { useState, useEffect } from 'react';
import { Search, X, Sparkles, Layers, FileText, Activity } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import {
  useVectorSearch,
  useTextSearchVectors,
  useHybridSearchVectors,
} from '@/hooks/useVectorSearch';

type SearchMode = 'vector' | 'text' | 'hybrid';

interface VectorSearchProps {
  storyId: string;
}

const MODE_CONFIG: Record<
  SearchMode,
  { label: string; icon: React.ElementType; description: string }
> = {
  vector: { label: '向量搜索', icon: Sparkles, description: '基于语义相似度检索相关场景' },
  text: { label: '文本搜索', icon: FileText, description: '基于关键词匹配（FTS5）精确检索' },
  hybrid: { label: '混合搜索', icon: Layers, description: '向量语义 + 关键词 RRF 融合排序' },
};

export function VectorSearch({ storyId }: VectorSearchProps) {
  const [query, setQuery] = useState('');
  const [mode, setMode] = useState<SearchMode>('vector');
  const [debouncedQuery, setDebouncedQuery] = useState('');

  const {
    results: vectorResults,
    isLoading: isVectorLoading,
    search: searchVector,
    clearResults: clearVectorResults,
  } = useVectorSearch();

  const { data: textResults = [], isLoading: isTextLoading } = useTextSearchVectors(
    storyId,
    debouncedQuery,
    5
  );

  const { data: hybridResults = [], isLoading: isHybridLoading } = useHybridSearchVectors(
    storyId,
    debouncedQuery,
    5
  );

  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedQuery(query.trim());
    }, 300);
    return () => clearTimeout(timer);
  }, [query]);

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    if (mode === 'vector') {
      searchVector({ story_id: storyId, query, top_k: 5 });
    }
  };

  const clearAll = () => {
    setQuery('');
    setDebouncedQuery('');
    clearVectorResults();
  };

  const results = mode === 'vector' ? vectorResults : mode === 'text' ? textResults : hybridResults;
  const isLoading =
    mode === 'vector' ? isVectorLoading : mode === 'text' ? isTextLoading : isHybridLoading;

  const ActiveIcon = MODE_CONFIG[mode].icon;

  return (
    <div className="space-y-4">
      {/* Mode Selector */}
      <div className="flex items-center gap-1 bg-cinema-800/50 rounded-lg p-1">
        {(Object.keys(MODE_CONFIG) as SearchMode[]).map(m => {
          const Icon = MODE_CONFIG[m].icon;
          return (
            <button
              key={m}
              type="button"
              onClick={() => {
                setMode(m);
                if (m !== 'vector') clearVectorResults();
              }}
              className={cn(
                'flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md text-xs font-medium transition-colors',
                mode === m
                  ? 'bg-cinema-700 text-white'
                  : 'text-gray-400 hover:text-white hover:bg-cinema-700/50'
              )}
              title={MODE_CONFIG[m].description}
            >
              <Icon className="w-3.5 h-3.5" />
              {MODE_CONFIG[m].label}
            </button>
          );
        })}
      </div>

      <form onSubmit={handleSearch} className="flex gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-500" />
          <input
            type="text"
            value={query}
            onChange={e => setQuery(e.target.value)}
            placeholder={MODE_CONFIG[mode].description}
            className="w-full pl-10 pr-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white placeholder-gray-500 focus:border-cinema-gold focus:outline-none"
          />
          {query && (
            <button
              type="button"
              onClick={clearAll}
              className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-500 hover:text-white"
            >
              <X className="w-4 h-4" />
            </button>
          )}
        </div>
        <Button type="submit" variant="primary" isLoading={isLoading} className="gap-2">
          <ActiveIcon className="w-4 h-4" />
          搜索
        </Button>
      </form>

      {results.length > 0 && (
        <div className="space-y-3">
          <p className="text-sm text-gray-400">
            {MODE_CONFIG[mode].label} 找到 {results.length} 个相关结果
          </p>
          {results.map(result => (
            <Card key={result.id} className="hover:border-cinema-gold/30 transition-colors">
              <CardContent className="p-4">
                <div className="flex items-start justify-between gap-4">
                  <div className="flex-1">
                    <p className="text-sm text-cinema-gold mb-1">第 {result.chapter_number} 章</p>
                    <p className="text-gray-300 text-sm line-clamp-3">{result.text}</p>
                  </div>
                  <span className="text-xs text-gray-500 shrink-0">
                    相关度: {(result.score * 100).toFixed(1)}%
                  </span>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}

function cn(...classes: (string | boolean | undefined)[]) {
  return classes.filter(Boolean).join(' ');
}
