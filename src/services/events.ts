import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { isTauri } from '@tauri-apps/api/core';
import { ApiClientError } from './client';
import {
  EVENTS,
  type EventEnvelope,
  type EventKey,
  type EventPayloads,
} from '../types/events';
export type EventSubscriber = (
  name: string,
  callback: (event: { payload: unknown }) => void,
) => Promise<UnlistenFn>;
/** Disposal handles an AbortSignal that fires while Tauri is still registering. */
export function createEventSubscription(
  subscribe: EventSubscriber,
  available: () => boolean,
) {
  return async <K extends EventKey>(
    key: K,
    callback: (event: EventEnvelope<EventPayloads[K]>) => void,
    signal?: AbortSignal,
  ): Promise<UnlistenFn> => {
    if (signal?.aborted) return () => {};
    if (!available())
      throw new ApiClientError(
        'DESKTOP_UNAVAILABLE',
        '浏览器预览不可订阅桌面任务事件。',
      );
    let disposed = false;
    let unlisten: UnlistenFn | undefined;
    const dispose = () => {
      if (disposed) return;
      disposed = true;
      signal?.removeEventListener('abort', dispose);
      unlisten?.();
    };
    signal?.addEventListener('abort', dispose, { once: true });
    try {
      unlisten = await subscribe(EVENTS[key], (event) => {
        if (!disposed)
          callback(event.payload as EventEnvelope<EventPayloads[K]>);
      });
      if (disposed) unlisten();
    } catch {
      dispose();
      throw new ApiClientError('INTERNAL_ERROR', '任务事件订阅失败，请重试。');
    }
    return dispose;
  };
}
export const subscribeEvent = createEventSubscription(listen, isTauri);
