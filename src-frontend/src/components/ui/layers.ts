/**
 * 层级契约（设计文档 §6）：全站 z-index 只允许取自这里，禁止魔法数。
 * nav 40 ｜ launcher/连接/更新 45 ｜ overlay 50 ｜ modal 60 ｜ toast 70
 */
export const Z_INDEX = {
  nav: 40,
  status: 45,
  overlay: 50,
  modal: 60,
  toast: 70,
} as const;
