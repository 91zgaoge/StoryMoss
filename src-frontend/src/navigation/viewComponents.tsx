import type { ComponentType } from 'react';
import type { ViewType } from '@/types';

import { Dashboard } from '@/pages/Dashboard';
import { Stories } from '@/pages/Stories';
import { Characters } from '@/pages/Characters';
import { Scenes } from '@/pages/Scenes';
import Corpus from '@/pages/Corpus';
import { WorldBuilding } from '@/pages/WorldBuilding';
import { KnowledgeGraph } from '@/pages/KnowledgeGraph';
import { Skills } from '@/pages/Skills';
import { Settings } from '@/pages/Settings';
import { BookDeconstruction } from '@/pages/BookDeconstruction';
import { Tasks } from '@/pages/Tasks';
import { CascadeCenter } from '@/pages/CascadeCenter';
import { Maintenance } from '@/pages/Maintenance';
import { Foreshadowing } from '@/pages/Foreshadowing';
import { NarrativeAnalysis } from '@/pages/NarrativeAnalysis';
import { StorySystem } from '@/pages/StorySystem';
import { Insights } from '@/pages/Insights';
import { IntentionGraphDiagnostics } from '@/pages/IntentionGraphDiagnostics';
import AgencyEval from '@/pages/AgencyEval';
import TrainingLab from '@/pages/TrainingLab';
import AgencyLearning from '@/pages/AgencyLearning';
import AgencyStudio from '@/pages/AgencyStudio';
import { Logs } from '@/pages/Logs';
import { TracingPanel } from '@/pages/TracingPanel';

function GuidebookDistillation() {
  return <BookDeconstruction initialTab="guidebooks" />;
}

/**
 * v0.97.0：renderView switch 的替代品。
 * `Record<ViewType, ComponentType>` 让 TypeScript 强制穷举——新增 ViewType
 * 若漏了页面映射会直接 tsc 失败，不再静默落到 Dashboard。
 *
 * 别名（mcp/writing-stats/creation-wizard）仍显式保留，但 UI 导航不再展示。
 */
export const VIEW_COMPONENTS: Record<ViewType, ComponentType> = {
  dashboard: Dashboard,
  stories: Stories,
  characters: Characters,
  world_building: WorldBuilding,
  scenes: Scenes,
  corpus: Corpus,
  'knowledge-graph': KnowledgeGraph,
  skills: Skills,
  mcp: Settings,
  'book-deconstruction': BookDeconstruction,
  'guidebook-distillation': GuidebookDistillation,
  tasks: Tasks,
  foreshadowing: Foreshadowing,
  'narrative-analysis': NarrativeAnalysis,
  'creation-wizard': Dashboard,
  'story-system': StorySystem,
  'usage-stats': Insights,
  'agency-eval': AgencyEval,
  'training-lab': TrainingLab,
  'agency-learning': AgencyLearning,
  'agency-studio': AgencyStudio,
  'writing-stats': Insights,
  'intention-graph': IntentionGraphDiagnostics,
  logs: Logs,
  tracing: TracingPanel,
  'cascade-center': CascadeCenter,
  maintenance: Maintenance,
  settings: Settings,
};
