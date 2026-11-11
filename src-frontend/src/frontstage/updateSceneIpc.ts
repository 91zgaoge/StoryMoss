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
  /** v0.96.0：加载时正文的 FNV-1a 指纹；内容保存用它做 CAS。 */
  expected_content_hash?: string | null;
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
  expectedContentHash?: string | null;
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
    expected_content_hash: params.expectedContentHash ?? null,
    updates,
  };
}

/**
 * v0.96.0 CAS 指纹：FNV-1a 64（UTF-8 bytes），与 Rust `content_hash` 同构。
 * 只比较正文，标题等元数据变化不会误判冲突。
 */
export function sceneContentHash(text: string): string {
  const bytes = new TextEncoder().encode(text);
  let hash = 0xcbf29ce484222325n;
  for (const byte of bytes) {
    hash ^= BigInt(byte);
    hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return hash.toString(16).padStart(16, '0');
}
