import type { PaletteId } from '../preview/palettes';
export interface AppSettings {
  theme: 'light' | 'dark';
  palette: PaletteId;
  glass_enabled: boolean;
  startup_page: 'home' | 'games';
  bangumi_cover_source: 'original' | 'hikarinagi';
  vndb_cover_source: 'original' | 'hikarinagi';
  tag_limit: number;
  launch_wait_seconds: number;
  ui_refresh_seconds: number;
  locale_emulator_path: string;
  magpie_path: string;
  default_locale_emulator: boolean;
  default_magpie: boolean;
  proxy_enabled: boolean;
  proxy_mode: 'system' | 'manual';
  proxy_url: string;
  sidebar_game_order: Record<string, string[]>;
}
export const DEFAULT_SETTINGS: AppSettings = {
  theme: 'light',
  palette: 'wisteria',
  glass_enabled: true,
  startup_page: 'home',
  bangumi_cover_source: 'hikarinagi',
  vndb_cover_source: 'hikarinagi',
  tag_limit: 10,
  launch_wait_seconds: 15,
  ui_refresh_seconds: 15,
  locale_emulator_path: '',
  magpie_path: '',
  default_locale_emulator: false,
  default_magpie: false,
  proxy_enabled: false,
  proxy_mode: 'system',
  proxy_url: '',
  sidebar_game_order: {},
};
