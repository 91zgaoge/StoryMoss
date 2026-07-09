import { describe, it, expect } from 'vitest';
import { buildUpdateSceneIpcArgs } from '../updateSceneIpc';

describe('buildUpdateSceneIpcArgs', () => {
  it('构造后端期望的 scene_id + updates 嵌套形状', () => {
    const args = buildUpdateSceneIpcArgs({
      sceneId: 'scene-1',
      title: '第一章',
      content: '<p>正文</p>',
    });
    expect(args).toEqual({
      scene_id: 'scene-1',
      updates: {
        title: '第一章',
        content: '<p>正文</p>',
      },
    });
    // 禁止旧的扁平错误形状
    expect(args).not.toHaveProperty('id');
    expect(args).not.toHaveProperty('content');
    expect(args).not.toHaveProperty('word_count');
  });

  it('空 title 不写入 updates，避免覆盖已有标题', () => {
    const args = buildUpdateSceneIpcArgs({
      sceneId: 'scene-2',
      title: '',
      content: '<p>x</p>',
    });
    const updates = args.updates as Record<string, unknown>;
    expect(updates.title).toBeUndefined();
    expect(updates.content).toBe('<p>x</p>');
  });
});
