import { useState } from 'react';
import { BookOpen, Globe, Users, List, GitBranch, Sparkles, ArrowRight } from 'lucide-react';
import type { BookAnalysisResult } from '@/types/book-deconstruction';
import { CharacterList } from './CharacterList';
import { ChapterOutline } from './ChapterOutline';
import { StoryArcView } from './StoryArcView';

interface BookDetailViewProps {
  analysis: BookAnalysisResult;
  onConvertToStory: () => void;
  isConverting: boolean;
}

type TabType = 'overview' | 'characters' | 'chapters' | 'story-arc';

export function BookDetailView({ analysis, onConvertToStory, isConverting }: BookDetailViewProps) {
  const [activeTab, setActiveTab] = useState<TabType>('overview');
  const { book, characters, scenes } = analysis;

  const tabs: { id: TabType; label: string; icon: React.ElementType }[] = [
    { id: 'overview', label: '概览', icon: BookOpen },
    { id: 'characters', label: '人物', icon: Users },
    { id: 'chapters', label: '章节', icon: List },
    { id: 'story-arc', label: '故事线', icon: GitBranch },
  ];

  const formatWordCount = (count?: number) => {
    if (!count) return '未知';
    if (count >= 10000) return `${(count / 10000).toFixed(1)}万字`;
    return `${count}字`;
  };

  return (
    <div className="flex flex-col h-full">
      {/* 头部信息 */}
      <div className="bg-cinema-900 border-b border-cinema-800 p-6">
        <div className="flex items-start justify-between">
          <div>
            <h2 className="text-2xl font-bold text-white mb-1">{book.title}</h2>
            <div className="flex items-center gap-4 text-sm text-gray-400">
              {book.author && <span>作者: {book.author}</span>}
              {book.genre && (
                <span className="px-2 py-0.5 rounded-full bg-cinema-gold/10 text-cinema-gold text-xs">
                  {book.genre}
                </span>
              )}
              <span>{formatWordCount(book.word_count)}</span>
            </div>
          </div>
          <button
            onClick={onConvertToStory}
            disabled={isConverting}
            className="flex items-center gap-2 px-4 py-2 rounded-xl bg-cinema-gold/20 text-cinema-gold border border-cinema-gold/30 hover:bg-cinema-gold/30 transition-colors disabled:opacity-50"
          >
            <Sparkles className="w-4 h-4" />
            <span className="text-sm font-medium">
              {isConverting ? '转换中...' : '一键转为故事'}
            </span>
            <ArrowRight className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* 标签页 */}
      <div className="flex border-b border-cinema-800 bg-cinema-950">
        {tabs.map(tab => {
          const Icon = tab.icon;
          const isActive = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`flex items-center gap-2 px-6 py-3 text-sm font-medium transition-colors ${
                isActive
                  ? 'text-cinema-gold border-b-2 border-cinema-gold'
                  : 'text-gray-500 hover:text-gray-300'
              }`}
            >
              <Icon className="w-4 h-4" />
              {tab.label}
            </button>
          );
        })}
      </div>

      {/* 内容区 */}
      <div className="flex-1 overflow-auto p-6">
        {activeTab === 'overview' && (
          <div className="space-y-6">
            {/* 基本信息卡片 */}
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
              <div className="bg-cinema-900 border border-cinema-800 rounded-xl p-4">
                <div className="text-xs text-gray-500 mb-1">小说类型</div>
                <div className="text-sm font-medium text-white">{book.genre || '未识别'}</div>
              </div>
              <div className="bg-cinema-900 border border-cinema-800 rounded-xl p-4">
                <div className="text-xs text-gray-500 mb-1">总字数</div>
                <div className="text-sm font-medium text-white">
                  {formatWordCount(book.word_count)}
                </div>
              </div>
              <div className="bg-cinema-900 border border-cinema-800 rounded-xl p-4">
                <div className="text-xs text-gray-500 mb-1">人物数</div>
                <div className="text-sm font-medium text-white">{characters.length} 人</div>
              </div>
              <div className="bg-cinema-900 border border-cinema-800 rounded-xl p-4">
                <div className="text-xs text-gray-500 mb-1">章节数</div>
                <div className="text-sm font-medium text-white">{scenes.length} 章</div>
              </div>
            </div>

            {/* 世界观 */}
            {book.world_setting && (
              <div className="bg-cinema-900 border border-cinema-800 rounded-xl p-4">
                <div className="flex items-center gap-2 mb-3">
                  <Globe className="w-4 h-4 text-cinema-gold" />
                  <h4 className="text-sm font-medium text-cinema-gold">世界观设定</h4>
                </div>
                <p className="text-sm text-gray-300 leading-relaxed whitespace-pre-wrap">
                  {book.world_setting}
                </p>
              </div>
            )}

            {/* 剧情概要 */}
            {book.plot_summary && (
              <div className="bg-cinema-900 border border-cinema-800 rounded-xl p-4">
                <div className="flex items-center gap-2 mb-3">
                  <BookOpen className="w-4 h-4 text-cinema-gold" />
                  <h4 className="text-sm font-medium text-cinema-gold">剧情概要</h4>
                </div>
                <p className="text-sm text-gray-300 leading-relaxed">{book.plot_summary}</p>
              </div>
            )}
          </div>
        )}

        {activeTab === 'characters' && <CharacterList characters={characters} />}
        {activeTab === 'chapters' && <ChapterOutline scenes={scenes} />}
        {activeTab === 'story-arc' && <StoryArcView book={book} scenes={scenes} />}
      </div>
    </div>
  );
}
