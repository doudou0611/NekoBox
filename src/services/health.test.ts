import { describe, expect, it } from 'vitest';
import { canInvokeDesktop, checkHealth } from './health';
describe('development connectivity', () => {
  it('does not pretend a browser is a desktop backend', () => {
    expect(canInvokeDesktop()).toBe(false);
  });
  it('rejects browser-only invocation explicitly', async () => {
    await expect(checkHealth()).rejects.toThrow('浏览器预览没有 Rust 后端');
  });
});
