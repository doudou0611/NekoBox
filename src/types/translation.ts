export interface TranslationSettings {
  enabled: boolean;
  languages: ('en' | 'ja' | 'all')[];
  fields: ('title' | 'description' | 'source_tags')[];
  endpoint: string;
  model: string;
  has_api_key: boolean;
}
export interface SaveTranslationSettings {
  enabled: boolean;
  languages: ('en' | 'ja' | 'all')[];
  fields: ('title' | 'description' | 'source_tags')[];
  endpoint: string;
  model: string;
  api_key: string | null;
  clear_api_key: boolean;
}
