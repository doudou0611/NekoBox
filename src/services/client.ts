import { invoke, isTauri } from '@tauri-apps/api/core';
import type {
  ApiResponse,
  CommandName,
  CommandPayloads,
  CommandResults,
  ErrorCode,
} from '../types/api';
import { ERROR_CODES } from '../types/api';
export class ApiClientError extends Error {
  constructor(
    public readonly code: ErrorCode,
    message: string,
  ) {
    super(message);
    this.name = 'ApiClientError';
  }
}
export function isValidRequestId(value: string): boolean {
  return /^[A-Za-z0-9_-]{1,128}$/.test(value);
}
export function validateResponse<T>(
  value: unknown,
  requestId: string,
): ApiResponse<T> {
  const invalid = () =>
    new ApiClientError(
      'INVALID_RESPONSE',
      '服务响应无效，请重试；本次响应未被应用。',
    );
  if (typeof value !== 'object' || value === null) throw invalid();
  const response = value as Record<string, unknown>;
  if (response.request_id !== requestId || typeof response.message !== 'string')
    throw invalid();
  if (
    response.success === true &&
    response.error_code === null &&
    response.data !== undefined &&
    response.data !== null
  )
    return value as ApiResponse<T>;
  if (
    response.success === false &&
    typeof response.error_code === 'string' &&
    ERROR_CODES.includes(response.error_code as ErrorCode) &&
    response.data === null
  )
    return value as ApiResponse<T>;
  throw invalid();
}
export interface DesktopTransport {
  available: () => boolean;
  invoke: (command: string, args: Record<string, unknown>) => Promise<unknown>;
  requestId: () => string;
}
export interface ApiOptions {
  signal?: AbortSignal;
}
const transport: DesktopTransport = {
  available: isTauri,
  invoke,
  requestId: () => crypto.randomUUID(),
};
export function createApiClient(adapter: DesktopTransport = transport) {
  return async <K extends CommandName>(
    command: K,
    payload: CommandPayloads[K],
    options: ApiOptions = {},
  ): Promise<ApiResponse<CommandResults[K]>> => {
    if (!adapter.available())
      throw new ApiClientError(
        'DESKTOP_UNAVAILABLE',
        '浏览器预览没有 Rust 后端，请在 Windows 使用 pnpm desktop:dev。',
      );
    const requestId = adapter.requestId();
    if (!isValidRequestId(requestId))
      throw new ApiClientError('INVALID_REQUEST', '请求标识无效。');
    const request =
      command === 'health_check'
        ? { request_id: requestId }
        : { request_id: requestId, payload };
    const signal = options.signal;
    if (signal && command !== 'search_metadata')
      throw new ApiClientError('INVALID_REQUEST', '该操作不支持搜索取消。');
    if (signal?.aborted)
      throw new ApiClientError('CANCELLED', '资料搜索已取消。');
    let abort: (() => void) | undefined;
    const cancellation = signal
      ? new Promise<never>((_, reject) => {
          abort = () => {
            const cancelId = adapter.requestId();
            if (isValidRequestId(cancelId)) {
              void adapter
                .invoke('cancel_metadata_search', {
                  request: {
                    request_id: cancelId,
                    payload: { search_request_id: requestId },
                  },
                })
                .catch(() => {});
            }
            reject(new ApiClientError('CANCELLED', '资料搜索已取消。'));
          };
          signal.addEventListener('abort', abort, { once: true });
        })
      : null;
    let response: unknown;
    try {
      const work = adapter.invoke(command, { request });
      response = await (cancellation
        ? Promise.race([work, cancellation])
        : work);
    } catch (cause) {
      if (cause instanceof ApiClientError && cause.code === 'CANCELLED')
        throw cause;
      throw new ApiClientError(
        'INTERNAL_ERROR',
        '桌面服务调用失败，请检查日志或重试；原始错误不会在此显示。',
      );
    } finally {
      if (abort) signal?.removeEventListener('abort', abort);
    }
    return validateResponse<CommandResults[K]>(response, requestId);
  };
}
export const invokeApi = createApiClient();
