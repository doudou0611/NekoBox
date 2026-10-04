import { open } from '@tauri-apps/plugin-dialog';
export async function chooseDatabaseExportDirectory(): Promise<string | null> {
  const path = await open({
    directory: true,
    multiple: false,
    title: '选择游戏库备份保存目录',
  });
  return typeof path === 'string' ? path : null;
}
export async function chooseDatabaseSnapshot(): Promise<string | null> {
  const path = await open({
    directory: false,
    multiple: false,
    title: '选择 NekoBox 数据库快照',
    filters: [
      { name: 'SQLite 数据库快照', extensions: ['sqlite3', 'sqlite', 'db'] },
    ],
  });
  return typeof path === 'string' ? path : null;
}
