<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch, useId } from 'vue';
import PreviewIcon from '../preview/PreviewIcon.vue';
const props = defineProps<{ open: boolean; title: string }>();
const emit = defineEmits<{ close: [] }>();
const dialog = ref<HTMLDialogElement>();
const headingId = useId();
let opener: HTMLElement | null = null;
watch(
  () => props.open,
  async (open) => {
    await nextTick();
    if (!dialog.value) return;
    if (open) {
      opener =
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
      dialog.value.showModal();
      dialog.value
        .querySelector<HTMLElement>('[data-autofocus],button')
        ?.focus();
    } else if (dialog.value.open) {
      dialog.value.close();
      if (opener?.isConnected) opener.focus();
    }
  },
);
onBeforeUnmount(() => {
  dialog.value?.close();
  if (opener?.isConnected) opener.focus();
});
</script>
<template>
  <Teleport to="body"
    ><dialog
      ref="dialog"
      class="home-dialog"
      :aria-labelledby="headingId"
      @cancel.prevent="emit('close')"
      @close="open && emit('close')"
    >
      <header>
        <h2 :id="headingId">{{ title }}</h2>
        <button
          type="button"
          class="icon-button"
          aria-label="关闭窗口"
          @click="emit('close')"
        >
          <PreviewIcon name="close" />
        </button>
      </header>
      <slot /></dialog
  ></Teleport>
</template>
<style scoped>
.home-dialog {
  max-width: 600px;
  width: calc(100% - 48px);
  max-height: 85dvh;
  padding: var(--space-24);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-xl);
  background: var(--surface);
  color: var(--text);
  box-shadow: var(--shadow-modal);
  overflow: auto;
}
.home-dialog[open] {
  animation: home-dialog-reveal var(--feedback-duration) var(--ease-standard)
    both;
}
.home-dialog::backdrop {
  background: rgb(0 0 0 / 36%);
}
header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 24px;
}
h2 {
  font-family: var(--font-display);
  font-size: 26px;
  margin: 0;
}
@keyframes home-dialog-reveal {
  from {
    opacity: 0;
    transform: translateY(12px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: none;
  }
}
:root[data-motion='reduced'] .home-dialog[open] {
  animation: none;
}
</style>
