/**
 * FrontStage 类型定义
 */

export interface HintPosition {
  line: number;
  column: number;
  offset: number;
}

export interface AiHint {
  id: string;
  text: string;
  position: HintPosition;
  duration: number;
  isPreview?: boolean;
}

export type FrontstageEvent =
  | { type: 'ContentUpdate'; payload: { text: string; chapter_id: string } }
  | { type: 'AiHint'; payload: { hint: string; position: HintPosition; duration_ms: number } }
  | { type: 'AiPreview'; payload: { text: string; insert_position: number } }
  | { type: 'ChapterSwitch'; payload: { story_id: string; chapter_id: string; title: string } }
  | { type: 'SaveStatus'; payload: { saved: boolean; timestamp?: string } };

export type BackstageEvent =
  | { type: 'ContentChanged'; payload: { text: string; chapter_id: string } }
  | { type: 'GenerationRequested'; payload: { chapter_id: string; context: string } }
  | { type: 'FrontstageClosed'; payload?: undefined }
  | { type: 'FrontstageFocused'; payload?: undefined };

export interface ChapterInfo {
  id: string;
  title: string;
  storyTitle?: string;
}
