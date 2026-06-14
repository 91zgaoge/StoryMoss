import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

describe('asyncTextAnalyzer', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal('Worker', undefined as unknown as typeof Worker);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('returns a valid PerceptionResult when Worker is unavailable (SSR/jsdom fallback)', async () => {
    const { analyzeTextAsync } = await import('../asyncTextAnalyzer');
    const html = '<p>第一句。</p><p>第二句内容。</p>';
    const result = await analyzeTextAsync(html);

    expect(result).toHaveProperty('totalChars');
    expect(result).toHaveProperty('paragraphs');
    expect(result).toHaveProperty('sentencePattern');
    expect(result).toHaveProperty('vocabulary');
    expect(result).toHaveProperty('pacing');
    expect(result).toHaveProperty('contentDistribution');
    expect(result.totalChars).toBeGreaterThan(0);
  });

  it('rejects immediately when the AbortSignal is already aborted', async () => {
    const { analyzeTextAsync } = await import('../asyncTextAnalyzer');
    const controller = new AbortController();
    controller.abort(new Error('already aborted'));

    await expect(analyzeTextAsync('<p>内容</p>', controller.signal)).rejects.toThrow(
      'already aborted'
    );
  });

  it('cancels an in-flight task and notifies the Worker', async () => {
    const postMessage = vi.fn();
    const workerInstance = {
      postMessage,
      onmessage: null as ((event: MessageEvent<unknown>) => void) | null,
      onerror: null as ((event: ErrorEvent) => void) | null,
    };

    class MockWorker {
      onmessage: ((event: MessageEvent<unknown>) => void) | null = null;
      onerror: ((event: ErrorEvent) => void) | null = null;
      constructor() {
        workerInstance.onmessage = (event: MessageEvent<unknown>) => {
          if (this.onmessage) this.onmessage(event);
        };
        workerInstance.onerror = (event: ErrorEvent) => {
          if (this.onerror) this.onerror(event);
        };
      }
      postMessage(...args: unknown[]) {
        postMessage(...args);
      }
    }

    vi.stubGlobal('Worker', MockWorker as unknown as typeof Worker);

    const { analyzeTextAsync } = await import('../asyncTextAnalyzer');
    const controller = new AbortController();
    const promise = analyzeTextAsync(
      '<p>较长的一段文本内容，用于测试取消逻辑。</p>',
      controller.signal
    );

    // 让 microtask 先注册任务并 postMessage
    await Promise.resolve();

    expect(postMessage).toHaveBeenCalledTimes(1);
    const analyzeCall = postMessage.mock.calls[0][0] as {
      type: string;
      id: number;
      htmlContent: string;
    };
    expect(analyzeCall.type).toBe('analyze');

    controller.abort();
    await Promise.resolve();

    // 取消时应向 Worker 发送 cancel 消息
    expect(postMessage).toHaveBeenCalledTimes(2);
    const cancelCall = postMessage.mock.calls[1][0] as { type: string; id: number };
    expect(cancelCall.type).toBe('cancel');
    expect(cancelCall.id).toBe(analyzeCall.id);

    // 任务被取消后，Promise 应 rejected
    await expect(promise).rejects.toBeDefined();
  });

  it('resolves when Worker returns a result before cancellation', async () => {
    const postMessage = vi.fn();
    const workerInstance = {
      postMessage,
      onmessage: null as ((event: MessageEvent<unknown>) => void) | null,
      onerror: null as ((event: ErrorEvent) => void) | null,
    };

    class MockWorker {
      onmessage: ((event: MessageEvent<unknown>) => void) | null = null;
      onerror: ((event: ErrorEvent) => void) | null = null;
      constructor() {
        workerInstance.onmessage = (event: MessageEvent<unknown>) => {
          if (this.onmessage) this.onmessage(event);
        };
        workerInstance.onerror = (event: ErrorEvent) => {
          if (this.onerror) this.onerror(event);
        };
      }
      postMessage(...args: unknown[]) {
        postMessage(...args);
      }
    }

    vi.stubGlobal('Worker', MockWorker as unknown as typeof Worker);

    const { analyzeTextAsync } = await import('../asyncTextAnalyzer');
    const promise = analyzeTextAsync('<p>内容</p>');

    await Promise.resolve();

    const analyzeCall = postMessage.mock.calls[0][0] as {
      type: string;
      id: number;
      htmlContent: string;
    };

    // 模拟 Worker 回传结果
    workerInstance.onmessage!(
      new MessageEvent('message', {
        data: {
          id: analyzeCall.id,
          result: {
            totalChars: 2,
            paragraphs: [],
            sentencePattern: {
              totalSentences: 0,
              avgLength: 0,
              shortSentenceRatio: 0,
              longSentenceRatio: 0,
              varietyIndex: 0,
              topStarters: [],
              isMonotonous: false,
            },
            vocabulary: {
              totalWords: 0,
              uniqueWords: 0,
              richness: 0,
              repeatedWords: [],
              hasRepetition: false,
              adjectiveDensity: 0,
              verbDensity: 0,
            },
            pacing: {
              variationScore: 0,
              paragraphVariation: 0,
              dialogueNarrativeAlternation: 0,
              currentPacing: 'steady',
              hasMonotonousSequence: false,
            },
            contentDistribution: {
              dialogue: 0,
              description: 0,
              narrative: 0,
              emotion: 0,
              dominant: 'narrative',
            },
            analyzedAt: Date.now(),
          },
        },
      })
    );

    const result = await promise;
    expect(result.totalChars).toBe(2);
  });
});
