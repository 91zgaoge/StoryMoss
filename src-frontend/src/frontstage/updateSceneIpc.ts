/**
 * 幕前/幕后共用的 update_scene IPC 参数构造。
 *
 * 后端签名：`update_scene(scene_id: String, updates: SceneUpdate)`
 * （见 src-tauri/src/scene_commands.rs）
 *
 * 历史 bug：FrontstageApp 曾传 `{ id, title, content, word_count }`，
 * 导致自动保存静默失败、「保存中」永不消失。
 */
export interface UpdateSceneIpcArgs {
  scene_id: string;
  updates: {
    title?: string;
    content?: string;
    [key: string]: unknown;
  };
}

export function buildUpdateSceneIpcArgs(params: {
  sceneId: string;
  title?: string | null;
  content?: string | null;
  extraUpdates?: Record<string, unknown>;
}): Record<string, unknown> {
  const updates: Record<string, unknown> = {
    ...(params.extraUpdates || {}),
  };
  if (params.title != null && params.title !== '') {
    updates.title = params.title;
  }
  if (params.content != null) {
    updates.content = params.content;
  }
  return {
    scene_id: params.sceneId,
    updates,
  };
}
