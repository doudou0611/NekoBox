import { convertFileSrc } from '@tauri-apps/api/core';
/** The backend exposes opaque registered IDs; Windows WebView2 needs the HTTP scheme form. */
export function screenshotResource(url: string) {
  const path = url.replace(/^screenshot:\/\/localhost\//, '');
  if (
    path === url ||
    !/^(?:thumbnail\/)?[0-9a-f-]{36}(?:\/[0-9a-f]{64}\.png)?$/.test(path) ||
    (path.startsWith('thumbnail/') && !path.endsWith('.png')) ||
    (!path.startsWith('thumbnail/') && path.includes('/'))
  )
    return '';
  // Tauri encodes the entire input, including slashes. Convert only the first
  // segment so the protocol handler receives its validated route segments.
  const [first, ...rest] = path.split('/');
  return [convertFileSrc(first, 'screenshot'), ...rest].join('/');
}
