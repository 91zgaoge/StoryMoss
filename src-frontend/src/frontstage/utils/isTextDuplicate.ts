/**
 * v0.26.8: 纯文本去重检测。
 * 用于判断 `generatedText` 是否已经包含在编辑器现有内容中，
 * 防止 DB 正文与幽灵文本叠加显示。
 */
export const normalizeForDuplicateCheck = (s: string): string => {
  return s
    .replace(/<[^>]*>/g, '')
    .replace(/\s+/g, '')
    .replace(
      /[\u3002\uff01\uff1f.!?，、；：""''（）《》\[\]【】…—～·\u201c\u201d\u2018\u2019]/g,
      ''
    );
};

export const isTextDuplicate = (existingText: string, generatedText: string): boolean => {
  const trimmedExisting = existingText.trim();
  const trimmedGenerated = generatedText.trim();
  if (!trimmedExisting || !trimmedGenerated) return false;

  const normalizedExisting = normalizeForDuplicateCheck(trimmedExisting);
  const normalizedGenerated = normalizeForDuplicateCheck(trimmedGenerated);
  if (!normalizedExisting || !normalizedGenerated) return false;

  const fingerprintLen = Math.min(500, normalizedGenerated.length);
  const generatedFingerprint = normalizedGenerated.slice(0, fingerprintLen);

  return (
    normalizedExisting.includes(generatedFingerprint) ||
    (normalizedExisting.length >= normalizedGenerated.length * 0.9 &&
      normalizedGenerated.includes(
        normalizedExisting.slice(0, Math.min(200, normalizedExisting.length))
      ))
  );
};
