/**
 * 统一实时状态同步中心 Hook
 *
 * 监听后端 `sync-event` 事件，自动刷新 TanStack Query 缓存，
 * 实现前后台数据自动对齐。
 *
 * 在 App.tsx（幕后）和 FrontstageApp.tsx（幕前）中各挂载一次，
 * 即可实现双向自动同步。
 */

import { useEffect, useRef } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useQueryClient } from '@tanstack/react-query';

// ==================== 类型定义 ====================

export interface SyncEventPayload {
  type: string;
  payload?: {
    storyId?: string;
    characterId?: string;
    sceneId?: string;
    chapterId?: string;
    title?: string;
    resourceType?: string;
  };
}

export interface SyncStoreOptions {
  /** 故事创建回调 */
  onStoryCreated?: (storyId: string, title?: string) => void;
  /** 故事更新回调 */
  onStoryUpdated?: (storyId: string, title?: string) => void;
  /** 故事删除回调 */
  onStoryDeleted?: (storyId: string) => void;
  /** 故事选择回调 */
  onStorySelected?: (storyId: string, title?: string) => void;
  /** 角色创建回调 */
  onCharacterCreated?: (storyId: string, characterId: string, name: string) => void;
  /** 角色更新回调 */
  onCharacterUpdated?: (characterId: string, name?: string) => void;
  /** 角色删除回调 */
  onCharacterDeleted?: (characterId: string) => void;
  /** 场景创建回调 */
  onSceneCreated?: (storyId: string, sceneId: string, title?: string) => void;
  /** 场景更新回调 */
  onSceneUpdated?: (storyId: string, sceneId: string, title?: string) => void;
  /** 场景删除回调 */
  onSceneDeleted?: (storyId: string, sceneId: string) => void;
  /** 场景选择回调 */
  onSceneSelected?: (storyId: string, sceneId: string, title?: string) => void;
  /** 章节创建回调 */
  onChapterCreated?: (storyId: string, chapterId: string, title?: string) => void;
  /** 章节更新回调 */
  onChapterUpdated?: (chapterId: string, title?: string) => void;
  /** 章节删除回调 */
  onChapterDeleted?: (chapterId: string) => void;
  /** 世界观更新回调 */
  onWorldBuildingUpdated?: (storyId: string) => void;
  /** 数据批量刷新回调 */
  onDataRefresh?: (storyId: string | undefined, resourceType: string) => void;
}

// ==================== Query Key 常量 ====================

const KEYS = {
  stories: ['stories'],
  characters: (storyId?: string) => storyId ? ['characters', storyId] : ['characters'],
  scenes: (storyId?: string) => storyId ? ['scenes', storyId] : ['scenes'],
  sceneDetail: (sceneId?: string) => ['scenes', 'detail', sceneId],
  chapters: (storyId?: string) => storyId ? ['chapters', storyId] : ['chapters'],
  chapterDetail: (chapterId?: string) => ['chapters', 'detail', chapterId],
  worldBuilding: (storyId?: string) => storyId ? ['world_building', storyId] : ['world_building'],
  foreshadowings: (storyId?: string) => storyId ? ['foreshadowings', storyId] : ['foreshadowings'],
  storyOutlines: (storyId?: string) => storyId ? ['story-outline', storyId] : ['story-outline'],
  knowledgeGraph: (storyId?: string) => storyId ? ['knowledge-graph', storyId] : ['knowledge-graph'],
  characterRelationships: (storyId?: string) => storyId ? ['character-relationships', storyId] : ['character-relationships'],
};

// ==================== Hook ====================

export function useSyncStore(options: SyncStoreOptions = {}) {
  const queryClient = useQueryClient();
  const optionsRef = useRef(options);
  optionsRef.current = options;

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    const setup = async () => {
      unlisten = await listen('sync-event', (event) => {
        const payload = (event.payload as any) || {};
        const { type } = payload;
        const p = payload.payload || payload; // 兼容直接 payload 和嵌套 payload

        const storyId = p?.storyId || p?.story_id;
        const characterId = p?.characterId || p?.character_id;
        const sceneId = p?.sceneId || p?.scene_id;
        const chapterId = p?.chapterId || p?.chapter_id;
        const title = p?.title;

        switch (type) {
          // === Story ===
          case 'storyCreated': {
            queryClient.invalidateQueries({ queryKey: KEYS.stories });
            optionsRef.current.onStoryCreated?.(storyId, title);
            break;
          }
          case 'storyUpdated': {
            queryClient.invalidateQueries({ queryKey: KEYS.stories });
            if (storyId) {
              queryClient.invalidateQueries({ queryKey: KEYS.scenes(storyId) });
              queryClient.invalidateQueries({ queryKey: KEYS.characters(storyId) });
              queryClient.invalidateQueries({ queryKey: KEYS.chapters(storyId) });
            }
            optionsRef.current.onStoryUpdated?.(storyId, title);
            break;
          }
          case 'storyDeleted': {
            queryClient.invalidateQueries({ queryKey: KEYS.stories });
            if (storyId) {
              queryClient.removeQueries({ queryKey: KEYS.scenes(storyId) });
              queryClient.removeQueries({ queryKey: KEYS.characters(storyId) });
              queryClient.removeQueries({ queryKey: KEYS.chapters(storyId) });
            }
            optionsRef.current.onStoryDeleted?.(storyId);
            break;
          }
          case 'storySelected': {
            optionsRef.current.onStorySelected?.(storyId, title);
            break;
          }

          // === Character ===
          case 'characterCreated': {
            if (storyId) {
              queryClient.invalidateQueries({ queryKey: KEYS.characters(storyId) });
            }
            optionsRef.current.onCharacterCreated?.(storyId, characterId, p?.name);
            break;
          }
          case 'characterUpdated': {
            queryClient.invalidateQueries({ queryKey: KEYS.characters() });
            if (characterId) {
              queryClient.invalidateQueries({ queryKey: KEYS.characters().map((k, i) => i === 0 ? k : undefined).filter(Boolean) as string[] });
            }
            optionsRef.current.onCharacterUpdated?.(characterId, p?.name);
            break;
          }
          case 'characterDeleted': {
            queryClient.invalidateQueries({ queryKey: KEYS.characters() });
            optionsRef.current.onCharacterDeleted?.(characterId);
            break;
          }

          // === Scene ===
          case 'sceneCreated': {
            if (storyId) {
              queryClient.invalidateQueries({ queryKey: KEYS.scenes(storyId) });
            }
            optionsRef.current.onSceneCreated?.(storyId, sceneId, title);
            break;
          }
          case 'sceneUpdated': {
            if (storyId) {
              queryClient.invalidateQueries({ queryKey: KEYS.scenes(storyId) });
            }
            if (sceneId) {
              queryClient.invalidateQueries({ queryKey: KEYS.sceneDetail(sceneId) });
            }
            optionsRef.current.onSceneUpdated?.(storyId, sceneId, title);
            break;
          }
          case 'sceneDeleted': {
            if (storyId) {
              queryClient.invalidateQueries({ queryKey: KEYS.scenes(storyId) });
            }
            if (sceneId) {
              queryClient.removeQueries({ queryKey: KEYS.sceneDetail(sceneId) });
            }
            optionsRef.current.onSceneDeleted?.(storyId, sceneId);
            break;
          }
          case 'sceneSelected': {
            optionsRef.current.onSceneSelected?.(storyId, sceneId, title);
            break;
          }

          // === Chapter ===
          case 'chapterCreated': {
            if (storyId) {
              queryClient.invalidateQueries({ queryKey: KEYS.chapters(storyId) });
            }
            optionsRef.current.onChapterCreated?.(storyId, chapterId, title);
            break;
          }
          case 'chapterUpdated': {
            queryClient.invalidateQueries({ queryKey: KEYS.chapters() });
            if (chapterId) {
              queryClient.invalidateQueries({ queryKey: KEYS.chapterDetail(chapterId) });
            }
            // v5.2.0: chapter 更新会同步到关联 scene，刷新 scenes 缓存
            if (storyId) {
              queryClient.invalidateQueries({ queryKey: KEYS.scenes(storyId) });
            }
            optionsRef.current.onChapterUpdated?.(chapterId, title);
            break;
          }
          case 'chapterDeleted': {
            queryClient.invalidateQueries({ queryKey: KEYS.chapters() });
            if (chapterId) {
              queryClient.removeQueries({ queryKey: KEYS.chapterDetail(chapterId) });
            }
            optionsRef.current.onChapterDeleted?.(chapterId);
            break;
          }

          // === World Building ===
          case 'worldBuildingUpdated': {
            if (storyId) {
              queryClient.invalidateQueries({ queryKey: KEYS.worldBuilding(storyId) });
            }
            optionsRef.current.onWorldBuildingUpdated?.(storyId);
            break;
          }

          // === Data Refresh ===
          case 'dataRefresh': {
            const resourceType = p?.resourceType || p?.resource_type || 'all';
            switch (resourceType) {
              case 'stories':
                queryClient.invalidateQueries({ queryKey: KEYS.stories });
                break;
              case 'characters':
                queryClient.invalidateQueries({ queryKey: KEYS.characters(storyId) });
                break;
              case 'scenes':
                queryClient.invalidateQueries({ queryKey: KEYS.scenes(storyId) });
                break;
              case 'chapters':
                queryClient.invalidateQueries({ queryKey: KEYS.chapters(storyId) });
                break;
              case 'all':
              default:
                queryClient.invalidateQueries({ queryKey: KEYS.stories });
                if (storyId) {
                  queryClient.invalidateQueries({ queryKey: KEYS.scenes(storyId) });
                  queryClient.invalidateQueries({ queryKey: KEYS.characters(storyId) });
                  queryClient.invalidateQueries({ queryKey: KEYS.chapters(storyId) });
                  queryClient.invalidateQueries({ queryKey: KEYS.worldBuilding(storyId) });
                  queryClient.invalidateQueries({ queryKey: KEYS.foreshadowings(storyId) });
                  queryClient.invalidateQueries({ queryKey: KEYS.storyOutlines(storyId) });
                  queryClient.invalidateQueries({ queryKey: KEYS.knowledgeGraph(storyId) });
                  queryClient.invalidateQueries({ queryKey: KEYS.characterRelationships(storyId) });
                }
                break;
            }
            optionsRef.current.onDataRefresh?.(storyId, resourceType);
            break;
          }

          default:
            // 忽略未知事件
            break;
        }
      });
    };

    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, [queryClient]);
}

export default useSyncStore;
