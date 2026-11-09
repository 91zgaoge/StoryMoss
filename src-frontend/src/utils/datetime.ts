/**
 * v0.93.14: UI 时间显示统一入口。
 *
 * 时间口径（永久教训）：
 * - 任务 `tasks.*` / `task_logs.created_at`：后端 Local（RFC3339，+08:00）。
 * - SQLite `datetime('now','localtime')` 字段：`YYYY-MM-DD HH:MM:SS`（本地无时区）。
 * - `storymoss.log` / workflow log / traces：UTC ISO（`Z`）。
 *
 * 规则：UI 与诊断报告一律先按 Date 解析，再按浏览器本地时间显示；
 * **禁止对原始字符串做 slice**（曾把 UTC 时间当本地，差 8 小时，反复被用户
 * 报「任务进度时间与日志时间不一致」）。
 */
function pad(n: number, width = 2): string {
  return String(n).padStart(width, '0');
}

/** SQLite 本地时间串 `YYYY-MM-DD HH:MM:SS` 补成 ISO 本地形态（无时区按本地解析）。 */
function normalizeTimestamp(value: string): string {
  if (/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}(\.\d+)?$/.test(value.trim())) {
    return value.trim().replace(' ', 'T');
  }
  return value;
}

/** 本地日期时间：`YYYY-MM-DD HH:MM:SS`（浏览器本地时区）。 */
export function formatLocalDateTime(value?: string | null): string {
  if (!value) return '—';
  const date = new Date(normalizeTimestamp(value));
  if (Number.isNaN(date.getTime())) return value;
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(
    date.getHours()
  )}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

/** 本地时间：`HH:MM:SS`（浏览器本地时区）。 */
export function formatLocalTime(value?: string | null): string {
  if (!value) return '—';
  const date = new Date(normalizeTimestamp(value));
  if (Number.isNaN(date.getTime())) return value;
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}
