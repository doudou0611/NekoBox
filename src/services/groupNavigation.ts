import { homeSearchIntent, homeFavoriteIntent } from './homeNavigation';
import { preview } from '../stores/library';

/** Group navigation starts with every member visible, independent of old library filters. */
export function selectGalleryGroup(group_id = '') {
  homeSearchIntent.value = false;
  homeFavoriteIntent.value = false;
  preview.query = '';
  preview.status_filter = 'all';
  preview.gallery_view = 'grid';
  Object.assign(preview.gallery_state, {
    source: '',
    tag: '',
    year: '',
    developer: '',
    playtime: '',
    backup: '',
    multiple: '',
    recent: '',
    system: 'all',
    collection_id: group_id,
    scroll_top: 0,
  });
}
