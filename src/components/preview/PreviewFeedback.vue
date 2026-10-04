<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import { preview, dismissToast } from '../../preview/store';
import { desktop } from '../../stores/library';
import PreviewIcon from './PreviewIcon.vue';
const dialog_element = ref<HTMLElement | null>(null);
let previous_focus: HTMLElement | null = null;
let previous_opener: (() => HTMLElement | null) | undefined;
watch(
  () => preview.dialog,
  async (dialog) => {
    if (dialog) {
      previous_opener = dialog.opener;
      previous_focus =
        dialog.opener?.() ??
        (document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null);
      await nextTick();
      dialog_element.value?.querySelector<HTMLElement>('button')?.focus();
    } else {
      const target = previous_focus?.isConnected
        ? previous_focus
        : previous_opener?.();
      target?.focus({ preventScroll: true });
      previous_opener = undefined;
    }
  },
);
function confirm() {
  const action = preview.dialog?.action;
  preview.dialog = null;
  action?.();
}
function onKey(event: KeyboardEvent) {
  event.stopPropagation();
  if (event.key === 'Escape') {
    preview.dialog = null;
    return;
  }
  if (event.key !== 'Tab') return;
  const buttons = dialog_element.value?.querySelectorAll<HTMLElement>(
    'button:not(:disabled), [tabindex="0"]',
  );
  if (!buttons?.length) return;
  const first = buttons[0]!;
  const last = buttons[buttons.length - 1]!;
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}
</script>
<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="preview.dialog"
        class="modal-scrim"
        @click.self="preview.dialog = null"
      >
        <section
          ref="dialog_element"
          class="preview-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="preview-dialog-title"
          aria-describedby="preview-dialog-description"
          @keydown="onKey"
        >
          <span class="eyebrow">{{
            desktop ? 'LOCAL LIBRARY' : 'PREVIEW · LOCAL ONLY'
          }}</span>
          <h2 id="preview-dialog-title">{{ preview.dialog.title }}</h2>
          <p id="preview-dialog-description">
            {{ preview.dialog.description }}
          </p>
          <div class="dialog-actions">
            <button class="secondary-button" @click="preview.dialog = null">
              取消</button
            ><button class="primary-button" @click="confirm">
              {{ preview.dialog.confirm_label }}
            </button>
          </div>
        </section>
      </div>
    </Transition>
    <div class="toast-region" aria-live="polite" aria-relevant="additions">
      <TransitionGroup name="toast">
        <div
          v-for="toast in preview.toasts"
          :key="toast.toast_id"
          class="preview-toast"
          role="status"
        >
          <PreviewIcon name="info" />
          <p>{{ toast.message }}</p>
          <button
            class="icon-button"
            aria-label="关闭提示"
            @click="dismissToast(toast.toast_id)"
          >
            <PreviewIcon name="close" :size="16" />
          </button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>
