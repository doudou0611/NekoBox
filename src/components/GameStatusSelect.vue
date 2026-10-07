<script setup lang="ts">
import { ref } from 'vue';
import {
  SelectRoot,
  SelectTrigger,
  SelectPortal,
  SelectContent,
  SelectViewport,
  SelectGroup,
  SelectLabel,
  SelectItem,
  SelectItemText,
  SelectItemIndicator,
} from 'reka-ui';
import { STATUS_LABELS, type PreviewStatus } from '../preview/data';
import { setStatus } from '../stores/library';
import PreviewIcon from './preview/PreviewIcon.vue';

const props = defineProps<{ gameId: string; status: PreviewStatus }>();
const open = ref(false);
const saving = ref(false);
const options: { value: PreviewStatus; caption: string }[] = [
  { value: 'completed', caption: '故事已读完' },
  { value: 'playing', caption: '继续这段旅程' },
  { value: 'paused', caption: '暂时放下，留待以后' },
  { value: 'dropped', caption: '在这里告一段落' },
  { value: 'not_started', caption: '等待故事开始' },
];

async function changeStatus(value: unknown) {
  if (saving.value || value === props.status) return;
  const option = options.find((item) => item.value === value);
  if (!option) return;
  saving.value = true;
  try {
    // Keep the displayed value tied to the library's confirmed state.
    await setStatus(props.gameId, option.value);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <SelectRoot
    v-model:open="open"
    :model-value="status"
    :disabled="saving"
    @update:model-value="changeStatus"
  >
    <SelectTrigger
      class="secondary-button game-status-trigger"
      :data-status="status"
      aria-label="游玩状态"
      :aria-busy="saving"
    >
      <span class="game-status-dot" aria-hidden="true"></span>
      <span class="game-status-current">{{ STATUS_LABELS[status] }}</span>
      <PreviewIcon
        :name="saving ? 'clock' : 'chevron-down'"
        :size="16"
        class="game-status-chevron"
      />
    </SelectTrigger>
    <SelectPortal>
      <SelectContent
        class="game-status-menu"
        data-detail-overlay
        position="popper"
        align="start"
        :side-offset="8"
        :collision-padding="12"
        aria-label="选择游玩状态"
        @escape-key-down="(event) => event.stopPropagation()"
      >
        <SelectViewport>
          <SelectGroup>
            <SelectLabel class="game-status-heading">游玩状态</SelectLabel>
            <SelectItem
              v-for="option in options"
              :key="option.value"
              :value="option.value"
              :text-value="STATUS_LABELS[option.value]"
              :data-status="option.value"
              class="game-status-option"
            >
              <span class="game-status-dot" aria-hidden="true"></span>
              <span class="game-status-copy">
                <SelectItemText>{{
                  STATUS_LABELS[option.value]
                }}</SelectItemText>
                <small aria-hidden="true">{{ option.caption }}</small>
              </span>
              <SelectItemIndicator class="game-status-check">
                <PreviewIcon name="check" :size="16" />
              </SelectItemIndicator>
            </SelectItem>
          </SelectGroup>
        </SelectViewport>
      </SelectContent>
    </SelectPortal>
  </SelectRoot>
</template>

<style>
.game-status-trigger {
  min-width: 120px;
  gap: 10px;
  transition:
    background var(--micro-duration) var(--ease-standard),
    border-color var(--micro-duration) var(--ease-standard),
    box-shadow var(--micro-duration) var(--ease-standard);
}
.game-status-trigger[data-state='open'] {
  border-color: var(--accent-ink, var(--accent));
  background: var(--accent-wash);
}
.game-status-trigger:focus-visible {
  outline: 2px solid var(--accent-ink, var(--accent));
  outline-offset: 3px;
}
.game-status-trigger[disabled] {
  cursor: wait;
  opacity: 0.65;
}
.game-status-current {
  flex: 1;
  text-align: left;
}
.game-status-chevron {
  color: var(--muted);
  transition: transform var(--feedback-duration) var(--ease-standard);
}
.game-status-trigger[data-state='open'] .game-status-chevron {
  transform: rotate(180deg);
}
.game-status-dot {
  width: 7px;
  height: 7px;
  flex: 0 0 7px;
  border-radius: 50%;
  background: var(--subtle);
  box-shadow: 0 0 0 3px color-mix(in srgb, currentColor 5%, transparent);
}
[data-status='completed'] .game-status-dot {
  background: var(--success);
}
[data-status='playing'] .game-status-dot {
  background: var(--accent-ink, var(--accent));
}
[data-status='paused'] .game-status-dot,
[data-status='pending_confirmation'] .game-status-dot {
  background: var(--warning);
}
[data-status='dropped'] .game-status-dot {
  background: var(--danger);
}
.game-status-menu {
  z-index: var(--z-modal);
  width: 236px;
  max-width: calc(100vw - 24px);
  max-height: var(--reka-select-content-available-height);
  padding: 7px;
  overflow: hidden;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-menu);
  background: var(--surface);
  color: var(--text);
  box-shadow: var(--shadow-floating);
  transform-origin: var(--reka-select-content-transform-origin);
}
.game-status-heading {
  padding: 7px 12px 10px;
  color: var(--muted);
  font-size: 11px;
  letter-spacing: 0.12em;
}
.game-status-option {
  position: relative;
  display: flex;
  align-items: center;
  gap: 13px;
  min-height: 53px;
  padding: 9px 12px;
  border-radius: var(--radius-control);
  cursor: pointer;
  outline: none;
  user-select: none;
  transition:
    background var(--micro-duration) var(--ease-standard),
    color var(--micro-duration) var(--ease-standard);
}
.game-status-option[data-state='checked'] {
  background: var(--accent-wash);
  color: var(--accent-ink, var(--accent));
}
.game-status-option[data-highlighted] {
  background: var(--surface-hover);
}
.game-status-option[data-state='checked'][data-highlighted] {
  background: color-mix(in srgb, var(--accent-wash) 75%, var(--surface-hover));
}
.game-status-copy {
  display: grid;
  flex: 1;
  gap: 3px;
  font-size: 13px;
}
.game-status-copy small {
  color: var(--muted);
  font-size: var(--type-caption);
}
.game-status-check {
  display: flex;
}
.game-status-menu[data-state='open'] {
  animation: status-menu-in var(--feedback-duration) var(--ease-standard);
}
.game-status-menu[data-state='closed'] {
  animation: status-menu-out var(--micro-duration) var(--ease-exit);
}
@keyframes status-menu-in {
  from {
    opacity: 0;
    transform: translateY(-5px) scale(0.97);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}
@keyframes status-menu-out {
  to {
    opacity: 0;
    transform: translateY(-3px) scale(0.98);
  }
}
:root[data-motion='reduced'] .game-status-menu {
  animation: none;
}
:root[data-motion='reduced'] .game-status-trigger,
:root[data-motion='reduced'] .game-status-chevron,
:root[data-motion='reduced'] .game-status-option {
  transition: none;
}
@media (prefers-reduced-motion: reduce) {
  .game-status-menu[data-state] {
    animation: none;
  }
  .game-status-trigger,
  .game-status-chevron,
  .game-status-option {
    transition: none;
  }
}
</style>
