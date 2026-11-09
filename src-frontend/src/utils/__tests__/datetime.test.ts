/**
 * v0.93.14: 时间显示口径回归——禁止 raw slice，统一按本地时区解析。
 */
import { describe, expect, it } from 'vitest';
import { formatLocalDateTime, formatLocalTime } from '../datetime';

describe('datetime format', () => {
  it('SQLite 本地时间串按本地时间原样展示（含日期，不再只显示时分秒）', () => {
    expect(formatLocalDateTime('2026-11-09 14:37:59')).toBe('2026-11-09 14:37:59');
    expect(formatLocalTime('2026-11-09 14:37:59')).toBe('14:37:59');
  });

  it('RFC3339 UTC 串按浏览器本地时区换算，而不是 slice 原始字符串', () => {
    const iso = '2026-11-09T06:37:59Z';
    const date = new Date(iso);
    const expected = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(
      2,
      '0'
    )}-${String(date.getDate()).padStart(2, '0')} ${String(date.getHours()).padStart(
      2,
      '0'
    )}:${String(date.getMinutes()).padStart(2, '0')}:${String(date.getSeconds()).padStart(2, '0')}`;
    expect(formatLocalDateTime(iso)).toBe(expected);
  });

  it('非法时间串保持原样（回退不崩溃）', () => {
    expect(formatLocalDateTime('not-a-date')).toBe('not-a-date');
    expect(formatLocalDateTime(undefined)).toBe('—');
  });
});
