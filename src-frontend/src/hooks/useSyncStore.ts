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
import type { SyncEvent } from '@/generated/SyncEvent';

// ==================== 类型定义 ====================

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
  /** 章节 commit（含 projections）完成回调 */
  onChapterCommitted?: (
    storyId: string,
    chapterId: string,
    chapterNumber: number,
    projectionStatus: { [key: string]: string }
  ) => void;
  /** 世界观更新回调 */
  onWorldBuildingUpdated?: (storyId: string) => void;
  /** 世界观创建回调 */
  onWorldBuildingCreated?: (storyId: string, worldBuildingId: string) => void;
  /** 世界观删除回调 */
  onWorldBuildingDeleted?: (storyId: string, worldBuildingId: string) => void;
  /** 风格 DNA 更新回调 */
  onStyleDnaUpdated?: (storyId: string, styleDnaId: string) => void;
  /** 任务创建回调 */
  onTaskCreated?: (taskId: string, name: string) => void;
  /** 任务更新回调 */
  onTaskUpdated?: (taskId: string, status: string) => void;
  /** 任务完成回调 */
  onTaskCompleted?: (taskId: string, success: boolean) => void;
  /** 批注创建回调 */
  onAnnotationCreated?: (storyId: string, annotationId: string, sceneId: string) => void;
  /** 批注解决回调 */
  onAnnotationResolved?: (storyId: string, annotationId: string, sceneId: string) => void;
  /** 数据批量刷新回调 */
  onDataRefresh?: (storyId: string | undefined, resourceType: string) => void;
}

// ==================== 穷尽检查辅助函数 ====================

function assertUnreachable(x: never): never {
  throw new Error(`[useSyncStore] Unhandled SyncEvent type: ${JSON.stringify(x)}`);
}

// ==================== Query Key 常量 ====================

const KEYS = {
  stories: ['stories'],
  characters: (storyId?: string) => (storyId ? ['characters', storyId] : ['characters']),
  scenes: (storyId?: string) => (storyId ? ['scenes', storyId] : ['scenes']),
  sceneDetail: (sceneId?: string) => ['scenes', 'detail', sceneId],
  chapters: (storyId?: string) => (storyId ? ['chapters', storyId] : ['chapters']),
  chapterDetail: (chapterId?: string) => ['chapters', 'detail', chapterId],
  worldBuilding: (storyId?: string) => (storyId ? ['world_building', storyId] : ['world_building']),
  foreshadowings: (storyId?: string) =>
    storyId ? ['foreshadowings', storyId] : ['foreshadowings'],
  storyOutlines: (storyId?: string) => (storyId ? ['story-outline', storyId] : ['story-outline']),
  knowledgeGraph: (storyId?: string) =>
    storyId ? ['knowledge-graph', storyId] : ['knowledge-graph'],
  characterRelationships: (storyId?: string) =>
    storyId ? ['character-relationships', storyId] : ['character-relationships'],
  payoffLedger: (storyId?: string) => (storyId ? ['payoff-ledger', storyId] : ['payoff-ledger']),
  storyTimeline: (storyId?: string) => (storyId ? ['story-timeline', storyId] : ['story-timeline']),
  tasks: ['tasks'],
  sceneAnnotations: (sceneId?: string) => ['scene-annotations', sceneId],
  storyUnresolvedAnnotations: (storyId?: string) => ['story-unresolved-annotations', storyId],
  textAnnotations: (scope?: string, id?: string) => ['text-annotations', scope, id],
  writingStyle: (storyId?: string) => (storyId ? ['writing_style', storyId] : ['writing_style']),
};

// ==================== Hook ====================

export function useSyncStore(options: SyncStoreOptions = {}) {
  const queryClient = useQueryClient();
  const optionsRef = useRef(options);
  optionsRef.current = options;

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    const setup = async () => {
      unlisten = await listen<SyncEvent>('sync-event', event => {
        const { type, payload } = event.payload;

        switch (type) {
          // === Story ===
          case 'storyCreated': {
            queryClient.invalidateQueries({ queryKey: KEYS.stories });
            optionsRef.current.onStoryCreated?.(payload.story_id, payload.title ?? undefined);
            break;
          }
          case 'storyUpdated': {
            queryClient.invalidateQueries({ queryKey: KEYS.stories });
            queryClient.invalidateQueries({ queryKey: KEYS.scenes(payload.story_id) });
            queryClient.invalidateQueries({ queryKey: KEYS.characters(payload.story_id) });
            queryClient.invalidateQueries({ queryKey: KEYS.chapters(payload.story_id) });
            optionsRef.current.onStoryUpdated?.(payload.story_id, payload.title ?? undefined);
            break;
          }
          case 'storyDeleted': {
            queryClient.invalidateQueries({ queryKey: KEYS.stories });
            queryClient.removeQueries({ queryKey: KEYS.scenes(payload.story_id) });
            queryClient.removeQueries({ queryKey: KEYS.characters(payload.story_id) });
            queryClient.removeQueries({ queryKey: KEYS.chapters(payload.story_id) });
            optionsRef.current.onStoryDeleted?.(payload.story_id);
            break;
          }
          case 'storySelected': {
            // v5.6.2 修复: 故事切换时自动刷新关联数据缓存，避免时序依赖
            queryClient.invalidateQueries({ queryKey: KEYS.characters(payload.story_id) });
            queryClient.invalidateQueries({ queryKey: KEYS.scenes(payload.story_id) });
            queryClient.invalidateQueries({ queryKey: KEYS.chapters(payload.story_id) });
            queryClient.invalidateQueries({ queryKey: KEYS.worldBuilding(payload.story_id) });
            queryClient.invalidateQueries({ queryKey: KEYS.foreshadowings(payload.story_id) });
            queryClient.invalidateQueries({ queryKey: KEYS.storyOutlines(payload.story_id) });
            queryClient.invalidateQueries({ queryKey: KEYS.knowledgeGraph(payload.story_id) });
            queryClient.invalidateQueries({
              queryKey: KEYS.characterRelationships(payload.story_id),
            });
            optionsRef.current.onStorySelected?.(payload.story_id, payload.title ?? undefined);
            break;
          }

          // === Character ===
          case 'characterCreated': {
            queryClient.invalidateQueries({ queryKey: KEYS.characters(payload.story_id) });
            optionsRef.current.onCharacterCreated?.(
              payload.story_id,
              payload.character_id,
              payload.name
            );
            break;
          }
          case 'characterUpdated': {
            if (payload.story_id) {
              queryClient.invalidateQueries({ queryKey: KEYS.characters(payload.story_id) });
            } else {
              queryClient.invalidateQueries({ queryKey: KEYS.characters() });
            }
            optionsRef.current.onCharacterUpdated?.(
              payload.character_id,
              payload.name ?? undefined
            );
            break;
          }
          case 'characterDeleted': {
            if (payload.story_id) {
              queryClient.invalidateQueries({ queryKey: KEYS.characters(payload.story_id) });
            } else {
              queryClient.invalidateQueries({ queryKey: KEYS.characters() });
            }
            optionsRef.current.onCharacterDeleted?.(payload.character_id);
            break;
          }

          // === Scene ===
          case 'sceneCreated': {
            queryClient.invalidateQueries({ queryKey: KEYS.scenes(payload.story_id) });
            // v5.6.1 修复: Scene 创建会关联/创建 Chapter，同步刷新 chapters 缓存
            queryClient.invalidateQueries({ queryKey: KEYS.chapters(payload.story_id) });
            optionsRef.current.onSceneCreated?.(
              payload.story_id,
              payload.scene_id,
              payload.title ?? undefined
            );
            break;
          }
          case 'sceneUpdated': {
            queryClient.invalidateQueries({ queryKey: KEYS.scenes(payload.story_id) });
            // P1-8 修复: Scene 更新会同步到关联 Chapter，刷新 chapters 缓存
            queryClient.invalidateQueries({ queryKey: KEYS.chapters(payload.story_id) });
            queryClient.invalidateQueries({ queryKey: KEYS.sceneDetail(payload.scene_id) });
            // P1-15 修复: 失效 useSceneWithChapter 的缓存
            queryClient.invalidateQueries({ queryKey: ['scenes', 'chapter', payload.scene_id] });
            optionsRef.current.onSceneUpdated?.(
              payload.story_id,
              payload.scene_id,
              payload.title ?? undefined
            );
            break;
          }
          case 'sceneDeleted': {
            queryClient.invalidateQueries({ queryKey: KEYS.scenes(payload.story_id) });
            // v5.6.1 修复: Scene 删除会清理 chapters.scene_id，同步刷新 chapters 缓存
            queryClient.invalidateQueries({ queryKey: KEYS.chapters(payload.story_id) });
            queryClient.removeQueries({ queryKey: KEYS.sceneDetail(payload.scene_id) });
            // P1-15 修复: 移除 useSceneWithChapter 的缓存
            queryClient.removeQueries({ queryKey: ['scenes', 'chapter', payload.scene_id] });
            optionsRef.current.onSceneDeleted?.(payload.story_id, payload.scene_id);
            break;
          }
          case 'sceneSelected': {
            optionsRef.current.onSceneSelected?.(
              payload.story_id,
              payload.scene_id,
              payload.title ?? undefined
            );
            break;
          }

          // === Chapter ===
          case 'chapterCreated': {
            queryClient.invalidateQueries({ queryKey: KEYS.chapters(payload.story_id) });
            optionsRef.current.onChapterCreated?.(
              payload.story_id,
              payload.chapter_id,
              payload.title ?? undefined
            );
            break;
          }
          case 'chapterUpdated': {
            queryClient.invalidateQueries({ queryKey: KEYS.chapters() });
            queryClient.invalidateQueries({ queryKey: KEYS.chapters(payload.story_id) });
            queryClient.invalidateQueries({ queryKey: KEYS.chapterDetail(payload.chapter_id) });
            // v5.2.0: chapter 更新会同步到关联 scene，刷新 scenes 缓存
            queryClient.invalidateQueries({ queryKey: KEYS.scenes(payload.story_id) });
            optionsRef.current.onChapterUpdated?.(payload.chapter_id, payload.title ?? undefined);
            break;
          }
          case 'chapterDeleted': {
            queryClient.invalidateQueries({ queryKey: KEYS.chapters() });
            queryClient.removeQueries({ queryKey: KEYS.chapterDetail(payload.chapter_id) });
            // P1-9 修复: Chapter 删除会清理 scenes.chapter_id，刷新 scenes 缓存
            queryClient.invalidateQueries({ queryKey: KEYS.scenes(payload.story_id) });
            optionsRef.current.onChapterDeleted?.(payload.chapter_id);
            break;
          }
          case 'chapterCommitted': {
            // v0.23.1: commit + projections 完成后刷新 read-model
            queryClient.invalidateQueries({ queryKey: KEYS.chapters(payload.story_id) });
            queryClient.invalidateQueries({ queryKey: KEYS.chapterDetail(payload.chapter_id) });
            queryClient.invalidateQueries({ queryKey: KEYS.knowledgeGraph(payload.story_id) });
            queryClient.invalidateQueries({ queryKey: KEYS.foreshadowings(payload.story_id) });
            queryClient.invalidateQueries({ queryKey: KEYS.payoffLedger(payload.story_id) });
            optionsRef.current.onChapterCommitted?.(
              payload.story_id,
              payload.chapter_id,
              payload.chapter_number,
              payload.projection_status
            );
            break;
          }

          // === World Building ===
          case 'worldBuildingUpdated': {
            queryClient.invalidateQueries({ queryKey: KEYS.worldBuilding(payload.story_id) });
            optionsRef.current.onWorldBuildingUpdated?.(payload.story_id);
            break;
          }
          case 'worldBuildingCreated': {
            queryClient.invalidateQueries({ queryKey: KEYS.worldBuilding(payload.story_id) });
            optionsRef.current.onWorldBuildingCreated?.(
              payload.story_id,
              payload.world_building_id
            );
            break;
          }
          case 'worldBuildingDeleted': {
            queryClient.invalidateQueries({ queryKey: KEYS.worldBuilding(payload.story_id) });
            optionsRef.current.onWorldBuildingDeleted?.(
              payload.story_id,
              payload.world_building_id
            );
            break;
          }

          // === Style DNA ===
          case 'styleDnaUpdated': {
            queryClient.invalidateQueries({ queryKey: KEYS.writingStyle(payload.story_id) });
            optionsRef.current.onStyleDnaUpdated?.(payload.story_id, payload.style_dna_id);
            break;
          }

          // === Task ===
          case 'taskCreated': {
            queryClient.invalidateQueries({ queryKey: KEYS.tasks });
            optionsRef.current.onTaskCreated?.(payload.task_id, payload.name);
            break;
          }
          case 'taskUpdated': {
            queryClient.invalidateQueries({ queryKey: KEYS.tasks });
            optionsRef.current.onTaskUpdated?.(payload.task_id, payload.status);
            break;
          }
          case 'taskCompleted': {
            queryClient.invalidateQueries({ queryKey: KEYS.tasks });
            optionsRef.current.onTaskCompleted?.(payload.task_id, payload.success);
            break;
          }

          // === Annotation ===
          case 'annotationCreated': {
            queryClient.invalidateQueries({ queryKey: KEYS.sceneAnnotations(payload.scene_id) });
            queryClient.invalidateQueries({
              queryKey: KEYS.storyUnresolvedAnnotations(payload.story_id),
            });
            queryClient.invalidateQueries({
              queryKey: KEYS.textAnnotations('scene', payload.scene_id),
            });
            optionsRef.current.onAnnotationCreated?.(
              payload.story_id,
              payload.annotation_id,
              payload.scene_id
            );
            break;
          }
          case 'annotationResolved': {
            queryClient.invalidateQueries({ queryKey: KEYS.sceneAnnotations(payload.scene_id) });
            queryClient.invalidateQueries({
              queryKey: KEYS.storyUnresolvedAnnotations(payload.story_id),
            });
            queryClient.invalidateQueries({
              queryKey: KEYS.textAnnotations('scene', payload.scene_id),
            });
            optionsRef.current.onAnnotationResolved?.(
              payload.story_id,
              payload.annotation_id,
              payload.scene_id
            );
            break;
          }

          // === v5.6.4 修复: 补全独立同步事件响应 ===
          case 'characterRelationshipsUpdated': {
            queryClient.invalidateQueries({
              queryKey: KEYS.characterRelationships(payload.story_id),
            });
            break;
          }
          case 'payoffLedgerUpdated': {
            queryClient.invalidateQueries({ queryKey: KEYS.payoffLedger(payload.story_id) });
            break;
          }
          case 'ingestionCompleted': {
            queryClient.invalidateQueries({ queryKey: KEYS.knowledgeGraph(payload.story_id) });
            break;
          }

          // === Data Refresh ===
          case 'dataRefresh': {
            const resourceType = payload.resource_type;
            const storyId = payload.story_id ?? undefined;
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
              case 'worldBuilding':
                queryClient.invalidateQueries({ queryKey: KEYS.worldBuilding(storyId) });
                break;
              // v5.6.2 修复: WritingStyle 更新后同时刷新 worldBuilding 和 writing_style 缓存
              case 'writingStyle':
                queryClient.invalidateQueries({ queryKey: KEYS.worldBuilding(storyId) });
                queryClient.invalidateQueries({ queryKey: ['writing_style', storyId] });
                break;
              // v5.6.1 修复: 大纲/伏笔更新后刷新缓存
              case 'storyOutlines':
                queryClient.invalidateQueries({ queryKey: KEYS.storyOutlines(storyId) });
                break;
              case 'foreshadowings':
                queryClient.invalidateQueries({ queryKey: KEYS.foreshadowings(storyId) });
                break;
              // v5.6.2 修复: 补充 knowledgeGraph 和 characterRelationships 单独刷新
              case 'knowledgeGraph':
                queryClient.invalidateQueries({ queryKey: KEYS.knowledgeGraph(storyId) });
                break;
              case 'characterRelationships':
                queryClient.invalidateQueries({ queryKey: KEYS.characterRelationships(storyId) });
                break;
              case 'payoffLedger':
                queryClient.invalidateQueries({ queryKey: KEYS.payoffLedger(storyId) });
                break;
              case 'storyTimeline':
                queryClient.invalidateQueries({ queryKey: KEYS.storyTimeline(storyId) });
                break;
              // v0.11.2: 模型配置变更时刷新 settings/models，让幕后立即感知新活跃模型
              case 'model_config':
              case 'app_settings':
                queryClient.invalidateQueries({ queryKey: ['settings'] });
                queryClient.invalidateQueries({ queryKey: ['models'] });
                break;
              case 'all':
              default: {
                // B1: 合并 9 次独立 invalidateQueries 为一次 predicate 批量刷新，
                // 减少 IPC/重渲染瀑布。若后端附带 affected_resources，只失效相关 key。
                const RESOURCE_KEY_MAP: Record<string, string> = {
                  stories: 'stories',
                  scenes: 'scenes',
                  characters: 'characters',
                  chapters: 'chapters',
                  worldBuilding: 'world_building',
                  foreshadowings: 'foreshadowings',
                  storyOutlines: 'story-outline',
                  knowledgeGraph: 'knowledge-graph',
                  characterRelationships: 'character-relationships',
                  payoffLedger: 'payoff-ledger',
                  storyTimeline: 'story-timeline',
                };
                const affectedResources = (payload as { affected_resources?: string[] })
                  .affected_resources;
                const resourcesToInvalidate =
                  affectedResources && affectedResources.length > 0
                    ? affectedResources.filter(r => r in RESOURCE_KEY_MAP)
                    : Object.keys(RESOURCE_KEY_MAP);

                if (resourcesToInvalidate.length > 0) {
                  const targetPrefixes = new Map<string, string | undefined>();
                  for (const r of resourcesToInvalidate) {
                    // stories 为全局 key，不跟随 storyId 过滤；其余按 storyId 过滤
                    targetPrefixes.set(RESOURCE_KEY_MAP[r], r === 'stories' ? undefined : storyId);
                  }
                  queryClient.invalidateQueries({
                    predicate: query => {
                      const key = query.queryKey;
                      if (!Array.isArray(key) || key.length === 0) return false;
                      const expectedStoryId = targetPrefixes.get(key[0]);
                      if (expectedStoryId === undefined) {
                        return targetPrefixes.has(key[0]);
                      }
                      return key[1] === expectedStoryId;
                    },
                  });
                }
                break;
              }
            }
            optionsRef.current.onDataRefresh?.(storyId, resourceType);
            break;
          }

          default:
            // 安全降级：遇到未处理的事件类型时记录 warn，不抛出异常
            // TypeScript 穷尽检查由编译时保障，运行时新增事件不会导致崩溃
            console.warn('[useSyncStore] Unhandled sync event type:', type);
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
