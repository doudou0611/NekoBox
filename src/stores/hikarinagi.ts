import { reactive } from 'vue';
import { api, desktop, errorText } from './library';
import type { HikarinagiSettings } from '../types/hikarinagi';

export const hikarinagi = reactive({
  settings: {
    enabled: true,
    method: 'client_credentials',
    client_id: '',
    has_client_secret: false,
    has_access_token: false,
    can_search: false,
  } as HikarinagiSettings,
  error: '',
});
export async function refreshHikarinagi() {
  if (!desktop) return;
  try {
    hikarinagi.settings = await api('get_hikarinagi_settings', {});
    hikarinagi.error = '';
  } catch (cause) {
    hikarinagi.settings.can_search = false;
    hikarinagi.error = errorText(cause);
  }
}
