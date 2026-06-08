import { loggedInvoke } from './core';
import type {
  Story,
  Character,
  Chapter,
  CreateStoryRequest,
  CreateCharacterRequest,
  UpdateChapterRequest,
} from '@/types/index';
import type {
  SceneAnnotation,
  TextAnnotation,
  ParagraphCommentary,
  SceneProposal,
  WorldBuildingOption,
  CharacterProfileOption,
  WritingStyleOption,
} from '@/types/v3';
// Health Check
export const healthCheck = () =>
  loggedInvoke<{ status: string; timestamp: string; version: string }>('health_check');

// Stories
export const listStories = () => loggedInvoke<Story[]>('list_stories');

export const createStory = (req: CreateStoryRequest) =>
  loggedInvoke<Story>('create_story', { ...req });

export const updateStory = (id: string, updates: Partial<Story>) =>
  loggedInvoke<void>('update_story', { id, ...updates });

export const deleteStory = (id: string) => loggedInvoke<void>('delete_story', { id });
// Characters
export const getStoryCharacters = (storyId: string) =>
  loggedInvoke<Character[]>('get_story_characters', { story_id: storyId });

export const createCharacter = (req: CreateCharacterRequest) =>
  loggedInvoke<Character>('create_character', { ...req });

export const updateCharacter = (id: string, updates: Partial<Character>) =>
  loggedInvoke<void>('update_character', { id, ...updates });

export const deleteCharacter = (id: string) => loggedInvoke<void>('delete_character', { id });

export interface CharacterQuickView {
  id: string;
  name: string;
  appearance_summary: string;
  status_tags: string[];
  last_seen_chapter: number;
}

export const getCharacterByName = (storyId: string, name: string) =>
  loggedInvoke<CharacterQuickView | null>('get_character_by_name', { story_id: storyId, name });
// Chapters
export const getStoryChapters = (storyId: string) =>
  loggedInvoke<Chapter[]>('get_story_chapters', { story_id: storyId });

export const getChapter = (id: string) => loggedInvoke<Chapter | null>('get_chapter', { id });

export const updateChapter = (id: string, updates: UpdateChapterRequest) =>
  loggedInvoke<void>('update_chapter', { id, ...updates });

export const deleteChapter = (id: string) => loggedInvoke<void>('delete_chapter', { id });

export const createChapter = (req: {
  story_id: string;
  chapter_number: number;
  title?: string;
  outline?: string;
  content?: string;
}) => loggedInvoke<Chapter>('create_chapter', { ...req });
export const createScene = (params: {
  story_id: string;
  chapter_id?: string;
  scene_number: number;
  title?: string;
  content?: string;
  outline?: string;
}) => loggedInvoke<import('@/types/v3').Scene>('create_scene', params);

export const getStoryScenes = (storyId: string) =>
  loggedInvoke<import('@/types/v3').Scene[]>('get_story_scenes', { story_id: storyId });

export const getScene = (sceneId: string) =>
  loggedInvoke<import('@/types/v3').Scene | null>('get_scene', { scene_id: sceneId });

export const updateScene = (
  sceneId: string,
  updates: {
    title?: string;
    content?: string;
    outline?: string;
    scene_number?: number;
  }
) => loggedInvoke<void>('update_scene', { scene_id: sceneId, ...updates });

export const deleteScene = (sceneId: string) =>
  loggedInvoke<void>('delete_scene', { scene_id: sceneId });

export const reorderScenes = (
  storyId: string,
  sceneOrders: Array<{ scene_id: string; new_number: number }>
) => loggedInvoke<void>('reorder_scenes', { story_id: storyId, scene_orders: sceneOrders });
// World Building
export const createWorldBuilding = (params: {
  story_id: string;
  category: string;
  title: string;
  content: string;
}) => loggedInvoke<import('@/types/v3').WorldBuilding>('create_world_building', params);

export const getWorldBuilding = (storyId: string) =>
  loggedInvoke<import('@/types/v3').WorldBuilding[]>('get_world_building', { story_id: storyId });

export const updateWorldBuilding = (
  worldBuildingId: string,
  updates: {
    category?: string;
    title?: string;
    content?: string;
  }
) =>
  loggedInvoke<void>('update_world_building', { world_building_id: worldBuildingId, ...updates });

export const deleteWorldBuilding = (worldBuildingId: string) =>
  loggedInvoke<void>('delete_world_building', { world_building_id: worldBuildingId });
// Writing Style
export const createWritingStyle = (params: {
  story_id: string;
  name: string;
  description?: string;
  style_rules?: string;
}) => loggedInvoke<import('@/types/v3').WritingStyle>('create_writing_style', params);

export const getWritingStyle = (storyId: string) =>
  loggedInvoke<import('@/types/v3').WritingStyle | null>('get_writing_style', {
    story_id: storyId,
  });

export const updateWritingStyle = (
  styleId: string,
  updates: {
    name?: string;
    description?: string;
    style_rules?: string;
  }
) => loggedInvoke<void>('update_writing_style', { style_id: styleId, ...updates });
