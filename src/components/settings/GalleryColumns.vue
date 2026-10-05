<script setup lang="ts">
import {
  SelectRoot,
  SelectTrigger,
  SelectPortal,
  SelectContent,
  SelectViewport,
  SelectItem,
  SelectItemText,
  SelectItemIndicator,
} from 'reka-ui';
import PreviewIcon from '../preview/PreviewIcon.vue';

defineProps<{ modelValue: number; disabled?: boolean }>();
const emit = defineEmits<{ 'update:modelValue': [value: number] }>();
const options = [3, 4, 5, 6, 7, 8, 9];
function change(value: unknown) {
  const columns = Number(value);
  if (options.includes(columns)) emit('update:modelValue', columns);
}
</script>

<template>
  <div class="gallery-columns-setting">
    <div class="gallery-columns-heading">
      <div>
        <h3>每行游戏数量</h3>
        <p>让封面舒展，或让更多故事同屏相遇。</p>
      </div>
      <SelectRoot
        :model-value="String(modelValue)"
        :disabled="disabled"
        @update:model-value="change"
      >
        <SelectTrigger
          class="secondary-button gallery-columns-trigger"
          aria-label="每行游戏数量"
        >
          <span>{{ modelValue }} 个 / 行</span>
          <PreviewIcon name="chevron-down" :size="16" />
        </SelectTrigger>
        <SelectPortal>
          <SelectContent
            class="gallery-columns-menu"
            position="popper"
            align="end"
            :side-offset="8"
            :collision-padding="12"
            aria-label="选择每行游戏数量"
          >
            <SelectViewport>
              <SelectItem
                v-for="count in options"
                :key="count"
                :value="String(count)"
                :text-value="`${count} 个 / 行`"
                class="gallery-columns-option"
              >
                <SelectItemText>{{ count }} 个 / 行</SelectItemText>
                <small v-if="count === 5">默认</small>
                <SelectItemIndicator>
                  <PreviewIcon name="check" :size="15" />
                </SelectItemIndicator>
              </SelectItem>
            </SelectViewport>
          </SelectContent>
        </SelectPortal>
      </SelectRoot>
    </div>
    <div class="gallery-columns-preview" aria-hidden="true">
      <span
        v-for="index in 9"
        :key="index"
        class="gallery-columns-mini"
        :class="{ active: index <= modelValue, last: index === modelValue }"
        :style="{ '--mini-hue': `${index * 18}deg` }"
        ><i
      /></span>
    </div>
    <p class="settings-explanation">
      保存后应用到游戏库网格，窄窗口会自动减少列数。
    </p>
  </div>
</template>

<style>
.gallery-columns-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 18px;
}
.gallery-columns-heading h3 {
  font-size: 13px;
  font-weight: 500;
}
.gallery-columns-heading p {
  margin-top: 7px;
  color: var(--muted);
  font-size: 11px;
  line-height: 1.7;
}
.gallery-columns-trigger {
  min-width: 136px;
  justify-content: space-between;
  transition:
    background var(--micro-duration) var(--ease-standard),
    border-color var(--micro-duration) var(--ease-standard);
}
.gallery-columns-trigger[data-state='open'] {
  background: var(--accent-wash);
  border-color: var(--accent-ink, var(--accent));
}
.gallery-columns-trigger svg {
  color: var(--muted);
  transition: transform var(--feedback-duration) var(--ease-standard);
}
.gallery-columns-trigger[data-state='open'] svg {
  transform: rotate(180deg);
}
.gallery-columns-menu {
  z-index: var(--z-modal);
  width: max(180px, var(--reka-select-trigger-width));
  max-width: calc(100vw - 24px);
  max-height: var(--reka-select-content-available-height);
  overflow: hidden;
  padding: 6px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  background: var(--surface);
  color: var(--text);
  box-shadow: var(--shadow-floating);
  transform-origin: var(--reka-select-content-transform-origin);
}
.gallery-columns-option {
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 40px;
  padding: 9px 12px;
  border-radius: var(--radius-md);
  font-size: 12px;
  outline: none;
  cursor: pointer;
  user-select: none;
  transition: background var(--micro-duration) var(--ease-standard);
}
.gallery-columns-option > span:first-child {
  flex: 1;
}
.gallery-columns-option small {
  font-size: 10px;
  color: var(--muted);
}
.gallery-columns-option[data-highlighted] {
  background: var(--surface-hover);
}
.gallery-columns-option[data-state='checked'] {
  background: var(--accent-wash);
  color: var(--accent-ink, var(--accent));
}
.gallery-columns-menu[data-state='open'] {
  animation: gallery-columns-in var(--feedback-duration) var(--ease-standard);
}
.gallery-columns-menu[data-state='closed'] {
  animation: gallery-columns-out var(--micro-duration) var(--ease-exit);
}
.gallery-columns-preview {
  display: flex;
  align-items: center;
  overflow: hidden;
  height: 120px;
  margin-top: 20px;
  padding: 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--accent-wash);
}
.gallery-columns-mini {
  flex: 0 0 0;
  min-width: 0;
  height: 84px;
  overflow: hidden;
  border-radius: 7px;
  opacity: 0;
  transform: translateY(10px);
  background: linear-gradient(
    155deg,
    color-mix(in srgb, var(--accent) 65%, var(--surface)),
    var(--surface) 85%
  );
  filter: hue-rotate(var(--mini-hue));
  transition:
    flex-grow var(--layout-duration) var(--ease-standard),
    margin-right var(--layout-duration) var(--ease-standard),
    opacity var(--feedback-duration) var(--ease-standard),
    transform var(--layout-duration) var(--ease-standard);
}
.gallery-columns-mini.active {
  flex-grow: 1;
  opacity: 1;
  transform: none;
  margin-right: 8px;
}
.gallery-columns-mini.last {
  margin-right: 0;
}
.gallery-columns-mini i {
  display: block;
  height: 3px;
  width: 45%;
  margin: 67px auto 0;
  border-radius: 3px;
  background: var(--accent);
  opacity: 0.35;
}
@keyframes gallery-columns-in {
  from {
    opacity: 0;
    transform: translateY(-5px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: none;
  }
}
@keyframes gallery-columns-out {
  to {
    opacity: 0;
    transform: translateY(-3px);
  }
}
:root[data-motion='reduced'] .gallery-columns-menu {
  animation: none;
}
:root[data-motion='reduced'] .gallery-columns-trigger,
:root[data-motion='reduced'] .gallery-columns-trigger svg,
:root[data-motion='reduced'] .gallery-columns-option,
:root[data-motion='reduced'] .gallery-columns-mini {
  transition: none;
}
@media (prefers-reduced-motion: reduce) {
  .gallery-columns-menu[data-state] {
    animation: none;
  }
  .gallery-columns-trigger,
  .gallery-columns-trigger svg,
  .gallery-columns-option,
  .gallery-columns-mini {
    transition: none;
  }
}
</style>
