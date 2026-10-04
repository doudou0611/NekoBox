import { reactive } from 'vue';
import { api, desktop } from './library';
import { DEFAULT_SETTINGS, type AppSettings } from '../types/settings';

export const appSettings = reactive({
  value: { ...DEFAULT_SETTINGS },
  loaded: !desktop,
});
let loading: Promise<void> | undefined;
export function loadAppSettings(): Promise<void> {
  if (!desktop || appSettings.loaded) return Promise.resolve();
  return (loading ??= api('get_app_settings', {})
    .then((value) => {
      appSettings.value = { ...DEFAULT_SETTINGS, ...value };
      appSettings.loaded = true;
    })
    .finally(() => {
      loading = undefined;
    }));
}
let saving: Promise<unknown> = Promise.resolve();
export async function saveAppSettings(
  value: AppSettings,
  keys: (keyof AppSettings)[] = Object.keys(value) as (keyof AppSettings)[],
) {
  const operation = saving
    .catch(() => {})
    .then(async () => {
      const next = { ...appSettings.value };
      for (const key of keys) Object.assign(next, { [key]: value[key] });
      appSettings.value = desktop
        ? await api('save_app_settings', next)
        : JSON.parse(JSON.stringify(next));
      return appSettings.value;
    });
  saving = operation;
  return operation;
}
