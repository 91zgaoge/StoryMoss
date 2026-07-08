import type { WritingStyleId } from '@/frontstage/config/writingStyles';

/**
 * 编辑器配置类型
 *
 * 从 components/EditorSettings 迁移至此，避免 stores 层依赖 components 层。
 */
export interface EditorConfig {
  styleId: WritingStyleId;
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  customFonts: CustomFont[];
}

export interface CustomFont {
  id: string;
  name: string;
  family: string;
  source: 'system' | 'google' | 'custom';
  url?: string;
}
