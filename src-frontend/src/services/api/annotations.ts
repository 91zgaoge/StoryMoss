import { loggedInvoke } from './core';
import type { SceneAnnotation, TextAnnotation, ParagraphCommentary } from '@/types/v3';
// Scene Annotations
export const createSceneAnnotation = (params: {
  scene_id: string;
  story_id: string;
  content: string;
  annotation_type: string;
}) => loggedInvoke<SceneAnnotation>('create_scene_annotation', params);

export const getSceneAnnotations = (sceneId: string) =>
  loggedInvoke<SceneAnnotation[]>('get_scene_annotations', { scene_id: sceneId });

export const getStoryUnresolvedAnnotations = (storyId: string) =>
  loggedInvoke<SceneAnnotation[]>('get_story_unresolved_annotations', { story_id: storyId });

export const updateSceneAnnotation = (annotationId: string, content: string) =>
  loggedInvoke<number>('update_scene_annotation', { annotation_id: annotationId, content });

export const resolveSceneAnnotation = (annotationId: string) =>
  loggedInvoke<number>('resolve_scene_annotation', { annotation_id: annotationId });

export const unresolveSceneAnnotation = (annotationId: string) =>
  loggedInvoke<number>('unresolve_scene_annotation', { annotation_id: annotationId });

export const deleteSceneAnnotation = (annotationId: string) =>
  loggedInvoke<number>('delete_scene_annotation', { annotation_id: annotationId });
// Text Inline Annotations
export const createTextAnnotation = (params: {
  story_id: string;
  scene_id?: string;
  chapter_id?: string;
  content: string;
  annotation_type: string;
  from_pos: number;
  to_pos: number;
}) => loggedInvoke<TextAnnotation>('create_text_annotation', params);

export const getTextAnnotationsByChapter = (chapterId: string) =>
  loggedInvoke<TextAnnotation[]>('get_text_annotations_by_chapter', { chapter_id: chapterId });

export const getTextAnnotationsByScene = (sceneId: string) =>
  loggedInvoke<TextAnnotation[]>('get_text_annotations_by_scene', { scene_id: sceneId });

export const updateTextAnnotation = (annotationId: string, content: string) =>
  loggedInvoke<number>('update_text_annotation', { annotation_id: annotationId, content });

export const resolveTextAnnotation = (annotationId: string) =>
  loggedInvoke<number>('resolve_text_annotation', { annotation_id: annotationId });

export const unresolveTextAnnotation = (annotationId: string) =>
  loggedInvoke<number>('unresolve_text_annotation', { annotation_id: annotationId });

export const deleteTextAnnotation = (annotationId: string) =>
  loggedInvoke<number>('delete_text_annotation', { annotation_id: annotationId });
// Commentator Agent
export const generateParagraphCommentaries = (params: {
  story_id: string;
  story_title: string;
  genre: string;
  text: string;
}) => loggedInvoke<string>('generate_paragraph_commentaries', params);
