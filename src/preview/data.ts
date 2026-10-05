export type PreviewStatus =
  | 'not_started'
  | 'playing'
  | 'paused'
  | 'completed'
  | 'dropped'
  | 'pending_confirmation';
export interface PreviewGame {
  hikari_field?: { app_id: number; released: boolean } | null;
  game_id: string;
  title: string;
  subtitle: string;
  developer: string;
  publisher?: string;
  release_date?: string;
  source_rating?: number | null;
  source_tags?: string[];
  cover_url: string;
  accent: string;
  status: PreviewStatus;
  favorite: boolean;
  duration_minutes: number;
  added_order: number;
  tags: string[];
  description: string;
  year: string;
  sources: string[];
  last_played_order: number;
  tag_ids?: string[];
  metadata_pending?: boolean;
  has_save_backup?: boolean;
  install_count?: number;
  launchable?: boolean;
  metadata_incomplete?: boolean;
  playtime_seconds?: number;
}
export const STATUS_LABELS: Record<PreviewStatus, string> = {
  not_started: '未开始',
  playing: '在玩',
  paused: '搁置',
  completed: '通关',
  dropped: '弃坑',
  pending_confirmation: '待确认入口',
};
/** Fictional titles and self-authored SVG artwork. Never production library data. */
export const PREVIEW_GAMES: PreviewGame[] = [
  {
    game_id: 'demo-shore',
    title: '潮汐寄来的信',
    subtitle: 'LETTERS FROM THE TIDE',
    developer: '虚构工作室 · 海岸',
    cover_url: '/preview/shore.svg',
    accent: '#c0b2ff',
    status: 'playing',
    favorite: true,
    duration_minutes: 482,
    added_order: 8,
    tags: ['海边', '日常', '微光'],
    description:
      '在漫长夏日的尽头，一封没有署名的信被潮水送到岸边。沿着海岸的旧铁道，与你一同寻找那些尚未说出口的话。\n\n这是为视觉原型创作的虚构作品。封面与文字均是离线演示内容，不代表真实游戏、实时资料或可执行安装。',
    year: '2024',
    sources: ['local'],
    last_played_order: 7,
  },
  {
    game_id: 'demo-forest',
    title: '月光落在森林里',
    subtitle: 'WHERE THE MOON RESTS',
    developer: '虚构工作室 · 森屿',
    cover_url: '/preview/forest.svg',
    accent: '#92c9c1',
    status: 'not_started',
    favorite: true,
    duration_minutes: 0,
    added_order: 7,
    tags: ['森林', '幻想', '旅途'],
    description:
      '穿过微光浮动的森林，在无人记得的车站，等待一班只在满月夜到来的列车。每一条小径，都藏着一段温柔的秘密。',
    year: '2023',
    sources: ['local'],
    last_played_order: 0,
  },
  {
    game_id: 'demo-sky',
    title: '与云同行的夏天',
    subtitle: 'A SUMMER ABOVE THE CLOUDS',
    developer: '虚构工作室 · 风铃',
    cover_url: '/preview/sky.svg',
    accent: '#f4b9aa',
    status: 'paused',
    favorite: false,
    duration_minutes: 138,
    added_order: 6,
    tags: ['夏日', '校园', '日常'],
    description:
      '山顶的天文台即将关闭。你和旧友决定留下最后一张星图，以及一个关于下个夏天的小小约定。',
    year: '2022',
    sources: ['local'],
    last_played_order: 5,
  },
  {
    game_id: 'demo-snow',
    title: '雪原的最后一盏灯',
    subtitle: 'THE LAST LIGHT OF WINTER',
    developer: '虚构工作室 · 白夜',
    cover_url: '/preview/snow.svg',
    accent: '#a9c7ee',
    status: 'completed',
    favorite: true,
    duration_minutes: 1096,
    added_order: 5,
    tags: ['冬日', '旅途', '微光'],
    description:
      '当雪覆盖了所有归途，远处的灯塔依旧亮着。这里的人们用故事取暖，也用故事告别。',
    year: '2021',
    sources: ['local'],
    last_played_order: 4,
  },
  {
    game_id: 'demo-garden',
    title: '花与未完成的诗',
    subtitle: 'THE GARDEN OF UNFINISHED VERSES',
    developer: '虚构工作室 · 花笺',
    cover_url: '/preview/garden.svg',
    accent: '#e6afd1',
    status: 'not_started',
    favorite: false,
    duration_minutes: 0,
    added_order: 4,
    tags: ['花园', '日常', '相遇'],
    description:
      '一座旧温室，一本写到一半的诗集。一场春雨之后，曾经错过的故事，又开始发芽。',
    year: '2020',
    sources: ['local'],
    last_played_order: 0,
  },
  {
    game_id: 'demo-amber',
    title: '黄昏收藏家',
    subtitle: 'THE KEEPER OF GOLDEN HOURS',
    developer: '虚构工作室 · 暮色',
    cover_url: '/preview/amber.svg',
    accent: '#e4bc8a',
    status: 'not_started',
    favorite: false,
    duration_minutes: 0,
    added_order: 3,
    tags: ['都市', '相遇', '旅途'],
    description:
      '在城市最古老的街角，有一家只在黄昏营业的店。店主不售卖商品，只替人收藏难以忘记的时光。',
    year: '2019',
    sources: ['local'],
    last_played_order: 2,
  },
];
export function createPreviewGames(): PreviewGame[] {
  return PREVIEW_GAMES.map((game) => ({ ...game, tags: [...game.tags] }));
}
export function formatPlaytime(minutes: number): string {
  return minutes
    ? `${Math.floor(minutes / 60)} 小时 ${minutes % 60} 分钟`
    : '故事尚未开始';
}
