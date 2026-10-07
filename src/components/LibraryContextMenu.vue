<script setup lang="ts">
import { nextTick, ref } from 'vue';
import type { GameMenuAction } from '../services/libraryContextActions';
import {
  ContextMenuRoot,
  ContextMenuTrigger,
  ContextMenuPortal,
  ContextMenuContent,
  ContextMenuItem,
} from 'reka-ui';
const props = defineProps<{
  label: string;
  disabled?: boolean;
  card?: boolean;
  game?: boolean;
  removable?: boolean;
}>();
const emit = defineEmits<{
  action: [action: GameMenuAction, opener: HTMLElement | null];
}>();
let trigger: HTMLElement | null = null;
const selected = ref(false);
const actions = [
  { id: 'rename' as const, label: '重命名' },
  { id: 'remove_from_group' as const, label: '移出当前组' },
  { id: 'add_to_group' as const, label: '添加到组' },
  { id: 'delete' as const, label: '删除' },
  { id: 'manage' as const, label: '管理' },
];
function remember(event: Event) {
  // The desktop's document listener suppresses native menus. Reka checks
  // defaultPrevented after nextTick, so consume propagation at this trigger.
  event.stopPropagation();
  if (props.disabled) {
    event.preventDefault();
    return;
  }
  const root = event.currentTarget as HTMLElement;
  trigger =
    root.querySelector<HTMLElement>('.group-open, [data-preview-open]') ?? root;
  selected.value = false;
}
function keyboardMenu(event: KeyboardEvent) {
  if (
    props.disabled ||
    !(event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey))
  )
    return;
  event.preventDefault();
  const root = event.currentTarget as HTMLElement;
  const bounds = root.getBoundingClientRect();
  root.dispatchEvent(
    new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: bounds.left + Math.min(bounds.width / 2, 32),
      clientY: bounds.top + Math.min(bounds.height / 2, 32),
    }),
  );
}
function select(action: GameMenuAction) {
  selected.value = true;
  void nextTick(() => {
    trigger?.focus({ preventScroll: true });
    emit('action', action, trigger);
  });
}
function restoreFocus(event: Event) {
  event.preventDefault();
  if (!selected.value) trigger?.focus({ preventScroll: true });
}
</script>
<template>
  <div class="library-context-anchor" :class="{ 'context-card': card }">
    <ContextMenuRoot>
      <ContextMenuTrigger
        as-child
        :disabled="disabled"
        @contextmenu="remember"
        @keydown="keyboardMenu"
      >
        <slot />
      </ContextMenuTrigger>
      <ContextMenuPortal>
        <ContextMenuContent
          class="library-context-menu"
          :aria-label="label"
          :collision-padding="8"
          @close-auto-focus="restoreFocus"
        >
          <ContextMenuItem
            v-for="item in actions.filter((item) =>
              item.id === 'add_to_group'
                ? game
                : item.id === 'remove_from_group'
                  ? game && removable
                  : true,
            )"
            :key="item.id"
            class="library-context-menu-item"
            :data-danger="item.id === 'delete'"
            @select="select(item.id)"
            >{{ item.label }}</ContextMenuItem
          >
        </ContextMenuContent>
      </ContextMenuPortal>
    </ContextMenuRoot>
  </div>
</template>
<style>
.library-context-anchor {
  display: contents;
}
.library-context-anchor.context-card {
  display: block;
  min-width: 0;
}
.library-context-menu {
  z-index: 220;
  min-width: 152px;
  max-width: calc(100vw - 16px);
  padding: 6px;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-menu);
  box-shadow: var(--shadow-floating);
  animation: library-menu-in var(--micro-duration) var(--ease-standard);
}
.library-context-menu-item {
  padding: 10px 14px;
  border-radius: var(--radius-control);
  cursor: pointer;
  outline: none;
  user-select: none;
}
.library-context-menu-item[data-highlighted] {
  background: var(--surface-hover);
}
.library-context-menu-item[data-danger='true'] {
  color: var(--danger);
}
@keyframes library-menu-in {
  from {
    opacity: 0;
    transform: translateY(3px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}
:root[data-motion='reduced'] .library-context-menu {
  animation: none;
}
</style>
