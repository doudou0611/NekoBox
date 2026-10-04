import { isTauri } from '@tauri-apps/api/core';
import { invokeApi } from './client';
export function canInvokeDesktop(): boolean {
  return isTauri();
}
export function checkHealth() {
  return invokeApi('health_check', undefined);
}
