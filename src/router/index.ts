import {
  createRouter,
  createWebHashHistory,
  type RouterHistory,
  type RouteRecordRaw,
} from 'vue-router';
import protocol from '../../shared/protocol.json';
const preview_pages: Record<
  string,
  NonNullable<RouteRecordRaw['component']>
> = {
  home: () => import('../views/PreviewHome.vue'),
  games: () => import('../views/PreviewGames.vue'),
  'game-detail': () => import('../views/PreviewDetail.vue'),
  activity: () => import('../views/Activity.vue'),
  saves: () => import('../views/Saves.vue'),
  settings: () => import('../views/PreviewSettings.vue'),
};
export const ROUTES: RouteRecordRaw[] = protocol.routes.map((route) => ({
  name: route.name,
  path: route.path,
  component: preview_pages[route.name],
  meta: { label: route.label, navigation: route.navigation, prototype: true },
}));
export const NAVIGATION = protocol.routes.filter((route) => route.navigation);
export function createAppRouter(
  history: RouterHistory = createWebHashHistory(),
) {
  return createRouter({
    history,
    routes: [...ROUTES, { path: '/:pathMatch(.*)*', redirect: '/' }],
  });
}
