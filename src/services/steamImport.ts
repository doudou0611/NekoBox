import type { GameInstallation } from '../types/domain';
export function installationLaunchable(
  i: Pick<
    GameInstallation,
    'path_valid' | 'executable_path' | 'source' | 'steam_app_id'
  >,
): boolean {
  return (
    i.path_valid &&
    (Boolean(i.executable_path) ||
      (i.source === 'steam' &&
        /^[1-9]\d*$/.test(i.steam_app_id ?? '') &&
        Number(i.steam_app_id) <= 4294967295))
  );
}
