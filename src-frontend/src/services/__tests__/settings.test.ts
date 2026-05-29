import { describe, it, expect, vi } from 'vitest';
import { getModelProviders, getProviderDefaultModels, setActiveModel } from '../settings';

// Mock Tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('getModelProviders', () => {
  it('should return all supported providers', () => {
    const providers = getModelProviders();
    const ids = providers.map(p => p.id);

    expect(ids).toContain('openai');
    expect(ids).toContain('anthropic');
    expect(ids).toContain('azure');
    expect(ids).toContain('ollama');
    expect(ids).toContain('deepseek');
    expect(ids).toContain('qwen');
    expect(ids).toContain('moonshot');
    expect(ids).toContain('zhipu');
    expect(ids).toContain('custom');
  });

  it('should indicate API key requirement correctly', () => {
    const providers = getModelProviders();
    const openai = providers.find(p => p.id === 'openai');
    const ollama = providers.find(p => p.id === 'ollama');
    const custom = providers.find(p => p.id === 'custom');

    expect(openai?.requiresApiKey).toBe(true);
    expect(ollama?.requiresApiKey).toBe(false);
    expect(custom?.requiresApiKey).toBe(false);
  });

  it('should list supported types for each provider', () => {
    const providers = getModelProviders();
    const openai = providers.find(p => p.id === 'openai');

    expect(openai?.supports).toContain('chat');
    expect(openai?.supports).toContain('embedding');
    expect(openai?.supports).toContain('multimodal');
    expect(openai?.supports).toContain('image');
  });
});

describe('getProviderDefaultModels', () => {
  it('should return OpenAI defaults', () => {
    const models = getProviderDefaultModels('openai');
    expect(models).toContain('gpt-4');
    expect(models).toContain('gpt-3.5-turbo');
    expect(models).toContain('dall-e-3');
  });

  it('should return Anthropic defaults', () => {
    const models = getProviderDefaultModels('anthropic');
    expect(models).toContain('claude-3-opus-20240229');
  });

  it('should return Ollama defaults', () => {
    const models = getProviderDefaultModels('ollama');
    expect(models).toContain('llama2');
    expect(models).toContain('mistral');
  });

  it('should return fallback for unknown provider', () => {
    const models = getProviderDefaultModels('unknown-provider');
    expect(models).toEqual(['custom-model']);
  });
});

describe('setActiveModel', () => {
  it('should invoke set_active_model with correct params', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue(undefined);

    await setActiveModel('chat', 'model-1');

    expect(invoke).toHaveBeenCalledWith('set_active_model', {
      modelType: 'chat',
      modelId: 'model-1',
    });
  });

  it('should invoke set_active_model for multimodal', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue(undefined);

    await setActiveModel('multimodal', 'gemma-4');

    expect(invoke).toHaveBeenCalledWith('set_active_model', {
      modelType: 'multimodal',
      modelId: 'gemma-4',
    });
  });

  it('should propagate invoke errors', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockRejectedValue(new Error('Backend error'));

    await expect(setActiveModel('chat', 'model-1')).rejects.toThrow('Backend error');
  });
});
