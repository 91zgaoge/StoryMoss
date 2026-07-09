import { create } from 'zustand';
import type { Story, Character, Chapter, Skill, ViewType, User } from '@/types/index';
import type { EditorConfig } from '@/types/editor';

interface AppState {
  // Navigation
  currentView: ViewType;
  setCurrentView: (view: ViewType) => void;

  // Current Story Context
  currentStory: Story | null;
  setCurrentStory: (story: Story | null) => void;

  // Current User
  currentUser: User | null;
  setCurrentUser: (user: User | null) => void;

  // Data
  stories: Story[];
  setStories: (stories: Story[]) => void;
  addStory: (story: Story) => void;
  updateStoryInList: (story: Story) => void;
  removeStory: (id: string) => void;

  characters: Character[];
  setCharacters: (characters: Character[]) => void;
  addCharacter: (character: Character) => void;
  updateCharacterInList: (character: Character) => void;
  removeCharacter: (id: string) => void;

  chapters: Chapter[];
  setChapters: (chapters: Chapter[]) => void;
  addChapter: (chapter: Chapter) => void;
  updateChapterInList: (chapter: Chapter) => void;
  removeChapter: (id: string) => void;

  skills: Skill[];
  setSkills: (skills: Skill[]) => void;
  updateSkill: (skill: Skill) => void;

  // Loading States
  isLoading: boolean;
  setIsLoading: (loading: boolean) => void;

  // Error
  error: string | null;
  setError: (error: string | null) => void;

  // W2-F2: 跨组件 UI 状态（替代 DOM CustomEvent）
  isLoginModalOpen: boolean;
  setLoginModalOpen: (open: boolean) => void;
  /** 导航高亮目标故事 ID（backstage-navigate-to-story 替代） */
  navigateHighlightStoryId: string | null;
  setNavigateHighlightStoryId: (id: string | null) => void;
  /** 编辑器配置（editor-config-changed 替代） */
  editorConfig: EditorConfig | null;
  setEditorConfig: (config: EditorConfig | null) => void;

  // L4 诊断页互链状态
  /** GenesisPanel 应自动选中的 session_id */
  selectedGenesisSessionId: string | null;
  setSelectedGenesisSessionId: (id: string | null) => void;
  /** TracingPanel 预过滤条件 */
  tracingFilter: { traceId?: string; sessionId?: string } | null;
  setTracingFilter: (filter: { traceId?: string; sessionId?: string } | null) => void;
  /** Logs 页面预填充搜索词 */
  logsSearchQuery: string | null;
  setLogsSearchQuery: (query: string | null) => void;

  /** Dashboard「快速创作」跳转 Stories 后自动触发 runCreationWorkflow */
  pendingQuickCreate: boolean;
  setPendingQuickCreate: (pending: boolean) => void;
  /** 角色页「关联场景」跳转到 Scenes 后选中的场景 ID */
  pendingSceneId: string | null;
  setPendingSceneId: (id: string | null) => void;
}

export const useAppStore = create<AppState>(set => ({
  // Navigation
  currentView: 'dashboard',
  setCurrentView: view => set({ currentView: view }),

  // Current Story
  currentStory: null,
  setCurrentStory: story => set({ currentStory: story }),

  // Current User
  currentUser: null,
  setCurrentUser: user => set({ currentUser: user }),

  // Stories
  stories: [],
  setStories: stories => set({ stories }),
  addStory: story =>
    set(state => ({
      stories: [...state.stories, story],
    })),
  updateStoryInList: story =>
    set(state => ({
      stories: state.stories.map(s => (s.id === story.id ? story : s)),
    })),
  removeStory: id =>
    set(state => ({
      stories: state.stories.filter(s => s.id !== id),
      currentStory: state.currentStory?.id === id ? null : state.currentStory,
    })),

  // Characters
  characters: [],
  setCharacters: characters => set({ characters }),
  addCharacter: character =>
    set(state => ({
      characters: [...state.characters, character],
    })),
  updateCharacterInList: character =>
    set(state => ({
      characters: state.characters.map(c => (c.id === character.id ? character : c)),
    })),
  removeCharacter: id =>
    set(state => ({
      characters: state.characters.filter(c => c.id !== id),
    })),

  // Chapters
  chapters: [],
  setChapters: chapters => set({ chapters }),
  addChapter: chapter =>
    set(state => ({
      chapters: [...state.chapters, chapter],
    })),
  updateChapterInList: chapter =>
    set(state => ({
      chapters: state.chapters.map(c => (c.id === chapter.id ? chapter : c)),
    })),
  removeChapter: id =>
    set(state => ({
      chapters: state.chapters.filter(c => c.id !== id),
    })),

  // Skills
  skills: [],
  setSkills: skills => set({ skills }),
  updateSkill: skill =>
    set(state => ({
      skills: state.skills.map(s => (s.id === skill.id ? skill : s)),
    })),

  // Loading
  isLoading: false,
  setIsLoading: loading => set({ isLoading: loading }),

  // Error
  error: null,
  setError: error => set({ error }),

  // W2-F2: 跨组件 UI 状态（替代 DOM CustomEvent）
  isLoginModalOpen: false,
  setLoginModalOpen: open => set({ isLoginModalOpen: open }),
  navigateHighlightStoryId: null,
  setNavigateHighlightStoryId: id => set({ navigateHighlightStoryId: id }),
  editorConfig: null,
  setEditorConfig: config => set({ editorConfig: config }),

  // L4 诊断页互链状态
  selectedGenesisSessionId: null,
  setSelectedGenesisSessionId: id => set({ selectedGenesisSessionId: id }),
  tracingFilter: null,
  setTracingFilter: filter => set({ tracingFilter: filter }),
  logsSearchQuery: null,
  setLogsSearchQuery: query => set({ logsSearchQuery: query }),

  pendingQuickCreate: false,
  setPendingQuickCreate: pending => set({ pendingQuickCreate: pending }),
  pendingSceneId: null,
  setPendingSceneId: id => set({ pendingSceneId: id }),
}));
