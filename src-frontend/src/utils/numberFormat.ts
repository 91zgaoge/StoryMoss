/**
 * 数字格式化工具函数
 *
 * 用于规范化浮点数精度、整数格式化等场景，
 * 避免 HTML number input 和 f32 序列化产生的精度噪声（如 0.8999999）
 */

/**
 * 将浮点数限制到指定小数位数
 * @param value 原始值
 * @param decimals 保留小数位数
 * @returns 规范化后的数值
 */
export function normalizeFloat(value: number, decimals: number = 2): number {
  if (!isFinite(value) || isNaN(value)) return 0;
  const factor = Math.pow(10, decimals);
  return Math.round(value * factor) / factor;
}

/**
 * 将值规范化为整数
 * @param value 原始值
 * @param min 最小值（可选）
 * @param max 最大值（可选）
 * @returns 规范化后的整数
 */
export function normalizeInt(value: number, min?: number, max?: number): number {
  let result = Math.round(value);
  if (!isFinite(result) || isNaN(result)) result = 0;
  if (min !== undefined) result = Math.max(min, result);
  if (max !== undefined) result = Math.min(max, result);
  return result;
}

/**
 * 格式化浮点数为展示字符串（去除末尾零）
 * @param value 原始值
 * @param decimals 最大保留小数位数
 * @returns 格式化后的字符串
 */
export function formatDisplayFloat(value: number, decimals: number = 2): string {
  if (!isFinite(value) || isNaN(value)) return '0';
  const normalized = normalizeFloat(value, decimals);
  // 转换为字符串并去除末尾零
  return parseFloat(normalized.toString()).toString();
}

/**
 * 限制数值在指定范围内
 * @param value 原始值
 * @param min 最小值
 * @param max 最大值
 * @returns 限制后的数值
 */
export function clampNumber(value: number, min: number, max: number): number {
  if (!isFinite(value) || isNaN(value)) return min;
  return Math.max(min, Math.min(max, value));
}

/**
 * 格式化延迟为带质量评级的展示文本
 * @param latencyMs 延迟毫秒数
 * @returns 如 "42ms · 优秀"
 */
export function formatLatencyWithQuality(latencyMs: number): string {
  if (latencyMs <= 0) return '未知';
  let quality: string;
  if (latencyMs < 100) {
    quality = '优秀';
  } else if (latencyMs < 300) {
    quality = '良好';
  } else {
    quality = '一般';
  }
  return `${latencyMs}ms · ${quality}`;
}
