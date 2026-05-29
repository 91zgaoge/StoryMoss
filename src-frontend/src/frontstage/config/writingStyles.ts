/**
 * 写作风格配置
 *
 * 定义不同的写作风格主题，包括字体、字号、行高、颜色等
 */

export type WritingStyleId = 'default' | 'classical' | 'modernCN' | 'minimal' | 'romantic';

export interface WritingStyle {
  id: WritingStyleId;
  name: string;
  description: string;
  author?: string;
  preview: string;
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  letterSpacing: string;
  paragraphSpacing: string;
  paperColor: string;
  inkColor: string;
  accentColor: string;
  quoteStyle: 'border' | 'italic' | 'both';
}

export const writingStyles: Record<WritingStyleId, WritingStyle> = {
  default: {
    id: 'default',
    name: '现代简洁',
    description: '清晰易读，适合日常创作',
    preview: '这是一个现代简洁的写作风格，适合大多数创作场景。',
    fontFamily: "'LXGW WenKai', 'Noto Serif SC', 'PingFang SC', 'Microsoft YaHei', serif",
    fontSize: 18,
    lineHeight: 1.8,
    letterSpacing: 'normal',
    paragraphSpacing: '1.5em',
    paperColor: 'oklch(96.5% 0.008 95)',
    inkColor: 'oklch(25% 0.015 85)',
    accentColor: 'oklch(58% 0.13 45)',
    quoteStyle: 'border',
  },

  classical: {
    id: 'classical',
    name: '古典深沉',
    description: '仿陀思妥耶夫斯基，深沉厚重',
    author: '陀思妥耶夫斯基风格',
    preview: '这是一种深沉而厚重的写作风格，文字中蕴含着复杂的人性探索与深刻的哲学思考。',
    fontFamily: "'Noto Serif SC', 'Source Han Serif CN', 'SimSun', serif",
    fontSize: 16,
    lineHeight: 1.9,
    letterSpacing: '0.02em',
    paragraphSpacing: '1.8em',
    paperColor: 'oklch(94% 0.01 95)',
    inkColor: 'oklch(22% 0.015 85)',
    accentColor: 'oklch(48% 0.13 55)',
    quoteStyle: 'italic',
  },

  modernCN: {
    id: 'modernCN',
    name: '现代中文',
    description: '仿张爱玲，优雅细腻',
    author: '张爱玲风格',
    preview: '文字如细密画，每一笔都精致入微，在平凡的生活中捕捉到那一瞬的动人光彩。',
    fontFamily: "'LXGW WenKai', 'ZCOOL XiaoWei', 'Noto Serif SC', serif",
    fontSize: 17,
    lineHeight: 1.85,
    letterSpacing: '0.03em',
    paragraphSpacing: '1.6em',
    paperColor: 'oklch(97% 0.01 95)',
    inkColor: 'oklch(32% 0.015 85)',
    accentColor: 'oklch(72% 0.1 75)',
    quoteStyle: 'both',
  },

  minimal: {
    id: 'minimal',
    name: '极简主义',
    description: '仿海明威，简洁有力',
    author: '海明威风格',
    preview: '文字简洁。句子很短。但有力。',
    fontFamily:
      "'SF Pro Display', 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', system-ui, sans-serif",
    fontSize: 20,
    lineHeight: 1.6,
    letterSpacing: '0.01em',
    paragraphSpacing: '2em',
    paperColor: 'oklch(100% 0 0)',
    inkColor: 'oklch(20% 0.01 0)',
    accentColor: 'oklch(25% 0.01 0)',
    quoteStyle: 'border',
  },

  romantic: {
    id: 'romantic',
    name: '浪漫抒情',
    description: '温暖柔和，情感丰富',
    preview: '文字如同清晨的露珠，带着柔和的光芒，在纸上轻轻流淌，诉说着内心深处的情感。',
    fontFamily: "'LXGW WenKai', 'Noto Serif SC', 'STKaiti', 'KaiTi', serif",
    fontSize: 19,
    lineHeight: 2.0,
    letterSpacing: '0.02em',
    paragraphSpacing: '2em',
    paperColor: 'oklch(98.5% 0.005 95)',
    inkColor: 'oklch(35% 0.01 85)',
    accentColor: 'oklch(78% 0.08 15)',
    quoteStyle: 'italic',
  },
};

export const defaultStyle: WritingStyle = writingStyles.default;

export const styleList = Object.values(writingStyles);
