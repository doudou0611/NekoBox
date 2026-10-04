import { describe, expect, it, vi } from 'vitest';
import { createApiClient, isValidRequestId, validateResponse } from './client';
const good = {
  success: true,
  error_code: null,
  message: 'ok',
  request_id: 'request-1',
  data: { version: '0.1.0', platform: 'fixture' },
};
describe('API boundary', () => {
  it('validates request IDs', () => {
    expect(isValidRequestId('request_1-test')).toBe(true);
    for (const value of ['', '中文', 'a\nb', '../file', 'a'.repeat(129)])
      expect(isValidRequestId(value)).toBe(false);
  });
  it('accepts consistent success and typed failure envelopes', () => {
    expect(validateResponse(good, 'request-1')).toEqual(good);
    const failure = {
      ...good,
      success: false,
      error_code: 'NOT_FOUND',
      data: null,
    };
    expect(validateResponse(failure, 'request-1')).toEqual(failure);
  });
  it.each([
    null,
    { ...good, request_id: 'wrong' },
    { ...good, data: null },
    { ...good, error_code: 'NOT_FOUND' },
    { ...good, success: false },
    { ...good, success: false, error_code: 'unexpected', data: null },
    { ...good, message: null },
  ])('rejects malformed response %#', (value) => {
    expect(() => validateResponse(value, 'request-1')).toThrow('服务响应无效');
  });
  it('preserves the health request and wraps business payloads', async () => {
    const call = vi.fn(async () => good);
    const client = createApiClient({
      available: () => true,
      invoke: call,
      requestId: () => 'request-1',
    });
    await client('health_check', undefined);
    expect(call).toHaveBeenLastCalledWith('health_check', {
      request: { request_id: 'request-1' },
    });
    await client('get_game', { game_id: 'fixture-game' });
    expect(call).toHaveBeenLastCalledWith('get_game', {
      request: {
        request_id: 'request-1',
        payload: { game_id: 'fixture-game' },
      },
    });
  });
  it('does not expose IPC errors or local paths', async () => {
    const client = createApiClient({
      available: () => true,
      requestId: () => 'request-1',
      invoke: async () => {
        throw Error('C:\\Private\\secret');
      },
    });
    await expect(client('health_check', undefined)).rejects.toThrow(
      '桌面服务调用失败',
    );
    await expect(client('health_check', undefined)).rejects.not.toThrow(
      'Private',
    );
  });
  it('never invokes in browser preview', async () => {
    const call = vi.fn();
    const client = createApiClient({
      available: () => false,
      invoke: call,
      requestId: () => 'request-1',
    });
    await expect(client('health_check', undefined)).rejects.toMatchObject({
      code: 'DESKTOP_UNAVAILABLE',
    });
    expect(call).not.toHaveBeenCalled();
  });
  it('cancels the original search once and rejects locally before its response arrives', async () => {
    let finish!: (value: unknown) => void;
    let sequence = 0;
    const call = vi.fn((command: string) =>
      command === 'search_metadata'
        ? new Promise<unknown>((resolve) => {
            finish = resolve;
          })
        : Promise.resolve({ ...good, request_id: 'request-2', data: true }),
    );
    const client = createApiClient({
      available: () => true,
      invoke: call,
      requestId: () => `request-${++sequence}`,
    });
    const controller = new AbortController();
    const work = client(
      'search_metadata',
      { query: '作品', providers: ['bangumi'] },
      { signal: controller.signal },
    );
    const rejected = expect(work).rejects.toMatchObject({ code: 'CANCELLED' });
    controller.abort();
    controller.abort();
    await rejected;
    expect(call).toHaveBeenCalledTimes(2);
    expect(call).toHaveBeenLastCalledWith('cancel_metadata_search', {
      request: {
        request_id: 'request-2',
        payload: { search_request_id: 'request-1' },
      },
    });
    finish({ ...good, data: [] });
    await Promise.resolve();
  });
  it('does not dispatch an aborted search, or cancel an already completed search', async () => {
    const call = vi.fn(async () => ({ ...good, data: [] }));
    const client = createApiClient({
      available: () => true,
      invoke: call,
      requestId: () => 'request-1',
    });
    const controller = new AbortController();
    controller.abort();
    await expect(
      client(
        'search_metadata',
        { query: '作品', providers: [] },
        { signal: controller.signal },
      ),
    ).rejects.toMatchObject({ code: 'CANCELLED' });
    expect(call).not.toHaveBeenCalled();
    const next = new AbortController();
    await client(
      'search_metadata',
      { query: '作品', providers: [] },
      { signal: next.signal },
    );
    next.abort();
    expect(call).toHaveBeenCalledTimes(1);
  });
});
