/**
 * v0.26.15: 纯文本去重检测重新导出。
 *
 * 实际实现已迁移到 src/utils/textCleanup.ts，供前端渲染层与自动排版共享。
 * 保留此文件以兼容现有调用方。
 */
export { normalizeForDuplicateCheck, isTextDuplicate } from '@/utils/textCleanup';
