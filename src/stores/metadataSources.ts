import { reactive } from 'vue';
import { api, desktop } from './library';
import type { MetadataProvider } from '../services/metadataScraper';
export interface MetadataSource {
  provider: MetadataProvider;
  enabled: boolean;
}
export const metadataSources = reactive({
  sources: ['hikarinagi', 'bangumi', 'vndb'].map((provider) => ({
    provider: provider as MetadataProvider,
    enabled: provider === 'hikarinagi',
  })),
  loaded: false,
});
export async function loadMetadataSources() {
  if (desktop) {
    const config = await api('get_metadata_sources', {});
    metadataSources.sources = config.sources as MetadataSource[];
  } else {
    try {
      const saved = JSON.parse(
        localStorage.getItem('galgame.preview.sources') || 'null',
      );
      if (
        Array.isArray(saved) &&
        saved.length === 3 &&
        saved.some((s) => s.enabled)
      )
        metadataSources.sources = saved;
    } catch {
      /* Optional preview preferences. */
    }
  }
  metadataSources.loaded = true;
}
export async function saveMetadataSources(sources: MetadataSource[]) {
  if (!sources.some((s) => s.enabled)) throw new Error('至少启用一个刮削源。');
  if (desktop) await api('save_metadata_sources', { sources });
  else localStorage.setItem('galgame.preview.sources', JSON.stringify(sources));
  metadataSources.sources = sources;
}
