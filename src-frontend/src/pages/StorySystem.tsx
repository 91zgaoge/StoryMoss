import { useState, useEffect } from 'react';
import { useAppStore } from '@/stores/appStore';
import { logFeatureUsage } from '@/services/tauri';
import {
  FileText,
  BookOpen,
  TrendingUp,
  Brain,
  ShieldAlert,
  Layers,
  Radar,
  AlertOctagon,
} from 'lucide-react';
import { ContractsTab } from './story-system/ContractsTab';
import { CommitsTab } from './story-system/CommitsTab';
import { ReadingPowerTab } from './story-system/ReadingPowerTab';
import { MemoryTab } from './story-system/MemoryTab';
import { AuditTab } from './story-system/AuditTab';
import { AntiAiTab } from './story-system/AntiAiTab';
import { GenresTab } from './story-system/GenresTab';
import { StyleDnaTab } from './story-system/StyleDnaTab';

type TabId =
  | 'contracts'
  | 'commits'
  | 'reading'
  | 'memory'
  | 'audit'
  | 'anti-ai'
  | 'genres'
  | 'style-dna';

export function StorySystem() {
  const currentStory = useAppStore(s => s.currentStory);
  const [activeTab, setActiveTab] = useState<TabId>('contracts');
  const [selectedChapter, setSelectedChapter] = useState<number>(1);

  useEffect(() => {
    if (!currentStory?.id) return;
    const featureMap: Record<string, string> = {
      contracts: 'story_contract',
      reading: 'reading_power',
      memory: 'memory_pack',
      audit: 'story_audit',
      'anti-ai': 'anti_ai_review',
      genres: 'genre_template',
    };
    const featureId = featureMap[activeTab];
    if (featureId) {
      logFeatureUsage(featureId, 'opened', currentStory.id);
    }
  }, [activeTab, currentStory?.id]);

  const tabs = [
    { id: 'contracts' as const, label: '合同', icon: FileText },
    { id: 'commits' as const, label: '提交链', icon: BookOpen },
    { id: 'reading' as const, label: '追读力', icon: TrendingUp },
    { id: 'memory' as const, label: '记忆', icon: Brain },
    { id: 'audit' as const, label: '审计', icon: ShieldAlert },
    { id: 'anti-ai' as const, label: 'Anti-AI', icon: AlertOctagon },
    { id: 'genres' as const, label: '体裁', icon: Layers },
    { id: 'style-dna' as const, label: '风格 DNA', icon: Radar },
  ];

  if (!currentStory) {
    return (
      <div className="flex items-center justify-center h-full text-gray-500">请先选择一个故事</div>
    );
  }

  return (
    <div className="h-full overflow-auto p-6">
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-white mb-1">故事合同</h1>
        <p className="text-gray-400 text-sm">{currentStory.title} — 合同驱动写作体系</p>
        <div
          className="mt-3 rounded-lg border border-cinema-gold/20 bg-cinema-gold/5 px-3 py-2 text-xs text-gray-300 leading-relaxed"
          data-testid="story-system-impact-callout"
        >
          <span className="text-cinema-gold font-medium">影响默认生成：</span>
          合同红线与运行时合同进入续写热路径（WriteTimeBundle）。「记忆」子 Tab 的语义事实主要在
          Full/改写路径注入；知识图谱实体摘要在默认续写中以轻量【相关设定】注入（top-5）。
        </div>
      </div>

      <div className="flex gap-2 mb-6 border-b border-cinema-800 pb-2">
        {tabs.map(tab => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
              activeTab === tab.id
                ? 'bg-cinema-gold/20 text-cinema-gold'
                : 'text-gray-400 hover:text-white hover:bg-cinema-800'
            }`}
          >
            <tab.icon className="w-4 h-4" />
            {tab.label}
          </button>
        ))}
      </div>

      {activeTab === 'contracts' && (
        <ContractsTab
          storyId={currentStory.id}
          selectedChapter={selectedChapter}
          onChapterChange={setSelectedChapter}
        />
      )}
      {activeTab === 'commits' && (
        <CommitsTab storyId={currentStory.id} selectedChapter={selectedChapter} />
      )}
      {activeTab === 'reading' && (
        <ReadingPowerTab
          storyId={currentStory.id}
          selectedChapter={selectedChapter}
          onChapterChange={setSelectedChapter}
        />
      )}
      {activeTab === 'memory' && (
        <MemoryTab storyId={currentStory.id} selectedChapter={selectedChapter} />
      )}
      {activeTab === 'audit' && <AuditTab storyId={currentStory.id} />}
      {activeTab === 'anti-ai' && (
        <AntiAiTab storyId={currentStory.id} genre={currentStory.genre} />
      )}
      {activeTab === 'genres' && <GenresTab />}
      {activeTab === 'style-dna' && <StyleDnaTab storyId={currentStory.id} />}
    </div>
  );
}
