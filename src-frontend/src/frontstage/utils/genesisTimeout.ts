/**
 * v0.30.4: 创世路径前端超时计算（纯函数，便于单测）。
 *
 * 创世（新小说）需要多次 LLM 调用（concept/assets/writing/review/assembly），
 * 本地慢模型易顶满后端 smart_execute 整体超时。前端超时必须 = 后端 + 30s 缓冲，
 * 保证后端先返回错误并落终态（finish_run），避免前端先杀后端（CANCELLATION）
 * 导致创世被砍掉无产出 + 僵尸 run 卡死故事续写。
 */

const DEFAULT_TIMEOUT_SECS = 600;
const CREATION_BUFFER_SECS = 30;

/**
 * 主超时（handleSmartGeneration）：创世用 beTimeout + 30s 缓冲，续写用 feTimeout。
 * v0.30.11: isBootstrap 现来自 LLM 意图分类（classifyIntent -> is_new_novel），
 * 不再依赖前端关键词列表（isNovelCreationIntent 已删除）。分类失败兜底为续写。
 */
export function genesisMainTimeoutSeconds(
  beTimeout: number | undefined,
  feTimeout: number | undefined,
  isBootstrap: boolean
): number {
  const be = beTimeout ?? DEFAULT_TIMEOUT_SECS;
  return isBootstrap ? be + CREATION_BUFFER_SECS : (feTimeout ?? DEFAULT_TIMEOUT_SECS);
}

/**
 * 看门狗超时（isGenerating useEffect）：创世进行中用 beTimeout + 30s，
 * 否则用 feTimeout。与主超时一致，覆盖 smart_execute 挂死、activity 残留、
 * 前端超时未触发等路径。isBootstrapInProgress 由 genesisDeliveryRef 三态
 * （idle/generating/delivered）判定：创世窗口内 !== 'idle' 为 true。
 */
export function watchdogTimeoutSeconds(
  beTimeout: number | undefined,
  feTimeout: number | undefined,
  isBootstrapInProgress: boolean
): number {
  const be = beTimeout ?? DEFAULT_TIMEOUT_SECS;
  return isBootstrapInProgress ? be + CREATION_BUFFER_SECS : (feTimeout ?? DEFAULT_TIMEOUT_SECS);
}
