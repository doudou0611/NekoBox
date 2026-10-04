export type PaletteId =
  | 'wisteria'
  | 'sea'
  | 'forest'
  | 'rose'
  | 'amber'
  | 'graphite'
  | 'black'
  | 'white'
  | 'jade'
  | 'coral';
type Colors = readonly [
  background: string,
  surface: string,
  hover: string,
  text: string,
  muted: string,
  subtle: string,
  accent: string,
  accent_text: string,
];
export interface Palette {
  id: PaletteId;
  name: string;
  description: string;
  dark: Colors;
  light: Colors;
}
export const PALETTES: Palette[] = [
  {
    id: 'wisteria',
    name: '紫藤',
    description: '柔和紫色 · 原始展厅',
    dark: [
      '#10121c',
      '#191c2a',
      '#242738',
      '#f0edf6',
      '#aaa7bb',
      '#79798e',
      '#c7b9f3',
      '#242135',
    ],
    light: [
      '#f3f0ed',
      '#fdfbf8',
      '#eae5e7',
      '#343044',
      '#6e6679',
      '#827d8c',
      '#776396',
      '#fbf8ff',
    ],
  },
  {
    id: 'sea',
    name: '海盐',
    description: '清透蓝色 · 海岸晨光',
    dark: [
      '#101923',
      '#192736',
      '#253b4d',
      '#edf5ff',
      '#a8bed1',
      '#7891aa',
      '#a3ceef',
      '#17354c',
    ],
    light: [
      '#eef4f8',
      '#f9fcff',
      '#e1ebf3',
      '#25384a',
      '#596e82',
      '#75889a',
      '#34688b',
      '#ffffff',
    ],
  },
  {
    id: 'forest',
    name: '森屿',
    description: '雾绿色 · 安静林间',
    dark: [
      '#111c18',
      '#1c2b24',
      '#2b3e33',
      '#eef7f0',
      '#b0c4b5',
      '#7d9787',
      '#aed6b6',
      '#203c2a',
    ],
    light: [
      '#eff4ed',
      '#fbfdf8',
      '#e2eadf',
      '#2c4030',
      '#607663',
      '#7b8e7c',
      '#4b7657',
      '#ffffff',
    ],
  },
  {
    id: 'rose',
    name: '樱霞',
    description: '玫瑰粉色 · 温柔暮光',
    dark: [
      '#21161e',
      '#30232d',
      '#44303f',
      '#fff0f7',
      '#d0b1c2',
      '#a17f93',
      '#eac0d5',
      '#4b253a',
    ],
    light: [
      '#f9eff3',
      '#fffbfd',
      '#f0e1e8',
      '#48303e',
      '#856579',
      '#a18497',
      '#975c7c',
      '#ffffff',
    ],
  },
  {
    id: 'amber',
    name: '琥珀',
    description: '暖金色 · 黄昏余温',
    dark: [
      '#201a13',
      '#2f271e',
      '#443729',
      '#fff5e7',
      '#cbb99d',
      '#9a866b',
      '#e5c28e',
      '#46321c',
    ],
    light: [
      '#f6f1e8',
      '#fffcf6',
      '#ece3d4',
      '#473a2b',
      '#7d6d54',
      '#96866f',
      '#88642f',
      '#ffffff',
    ],
  },
  {
    id: 'graphite',
    name: '石墨',
    description: '中性色 · 清晰阅读',
    dark: [
      '#17191c',
      '#23262b',
      '#34383f',
      '#f2f3f5',
      '#b4b8c0',
      '#858b97',
      '#c4cbd5',
      '#29313d',
    ],
    light: [
      '#f0f1f3',
      '#fdfdfe',
      '#e4e6eb',
      '#32363e',
      '#686f7b',
      '#858c96',
      '#53647c',
      '#ffffff',
    ],
  },
  {
    id: 'black',
    name: '黑色',
    description: '墨色强调 · 简洁有力',
    dark: [
      '#191a1d',
      '#25262b',
      '#35363c',
      '#f5f5f7',
      '#bec0c8',
      '#989ba6',
      '#000000',
      '#ffffff',
    ],
    light: [
      '#f0f0f0',
      '#ffffff',
      '#e5e5e5',
      '#202024',
      '#606067',
      '#777780',
      '#000000',
      '#ffffff',
    ],
  },
  {
    id: 'white',
    name: '白色',
    description: '白色强调 · 轻盈明亮',
    dark: [
      '#15171c',
      '#22252d',
      '#333743',
      '#f4f6fb',
      '#b6bdcc',
      '#949eb4',
      '#ffffff',
      '#17191f',
    ],
    light: [
      '#e9ebef',
      '#f4f5f7',
      '#d9dde4',
      '#242936',
      '#5b6373',
      '#778195',
      '#ffffff',
      '#20242c',
    ],
  },
  {
    id: 'jade',
    name: '青玉',
    description: '青绿强调 · 清凉澄澈',
    dark: [
      '#0d2023',
      '#153136',
      '#22454a',
      '#e4faf7',
      '#a3c6c5',
      '#799e9e',
      '#71d6cc',
      '#103b38',
    ],
    light: [
      '#edf6f3',
      '#fafffd',
      '#dfeee8',
      '#1c403b',
      '#52786f',
      '#6a8e86',
      '#16776d',
      '#ffffff',
    ],
  },
  {
    id: 'coral',
    name: '珊瑚',
    description: '橙红强调 · 海边夕照',
    dark: [
      '#25191a',
      '#352627',
      '#4b3635',
      '#fff2eb',
      '#d6b3ac',
      '#ad8b85',
      '#ffb29d',
      '#512a20',
    ],
    light: [
      '#faf0ea',
      '#fffcf9',
      '#f1e0d6',
      '#49352e',
      '#826a5e',
      '#9c8275',
      '#a75137',
      '#ffffff',
    ],
  },
];
function alpha(hex: string, opacity: number) {
  const value = Number.parseInt(hex.slice(1), 16);
  return `rgb(${value >> 16} ${(value >> 8) & 255} ${value & 255} / ${opacity}%)`;
}
export function paletteTokens(
  palette: Palette,
  mode: 'dark' | 'light',
): Record<string, string> {
  const [background, surface, hover, text, muted, subtle, accent, accent_text] =
    palette[mode];
  const light = mode === 'light';
  const materialOpacity = light ? 62 : 60;
  return {
    '--background': background,
    '--surface': surface,
    '--surface-hover': hover,
    '--text': text,
    '--muted': muted,
    '--subtle': subtle,
    '--accent': accent,
    '--accent-ink':
      palette.id === 'black' || palette.id === 'white' ? text : accent,
    '--accent-text': accent_text,
    '--surface-glass': alpha(surface, light ? 82 : 74),
    '--navigation-glass': alpha(background, 92),
    '--navigation-desktop-tint': alpha(background, materialOpacity),
    '--desktop-main-tint': alpha(background, materialOpacity),
    '--border': alpha(text, light ? 12 : 10),
    '--border-strong': alpha(text, 24),
    '--accent-wash': alpha(accent, light ? 10 : 13),
    '--halo': alpha(accent, 14),
    '--scrollbar-thumb': alpha(muted, 38),
    '--scrollbar-thumb-hover': alpha(accent, 68),
    '--swatch-dark': palette.dark[1],
    '--swatch-dark-tile': palette.dark[6],
    '--swatch-light': palette.light[0],
    '--swatch-light-tile': palette.light[6],
  };
}
