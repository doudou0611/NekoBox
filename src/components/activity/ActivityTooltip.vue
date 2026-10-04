<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
const props = defineProps<{
  target: Element | null;
  title: string;
  text: string;
  id: string;
}>();
const emit = defineEmits<{ dismiss: [] }>();
const layoutRevision = ref(0);
const position = computed(() => {
  void layoutRevision.value;
  if (!props.target) return {};
  const r = props.target.getBoundingClientRect();
  return {
    left: `${Math.max(12, Math.min(window.innerWidth - 252, r.left + r.width / 2 - 120))}px`,
    top: `${Math.min(window.innerHeight - 96, r.top > 152 ? r.top - 90 : r.bottom + 10)}px`,
  };
});
const hide = () => emit('dismiss');
const scroll = () => {
  if (props.target === document.activeElement) layoutRevision.value++;
  else hide();
};
const escape = (e: KeyboardEvent) => {
  if (e.key === 'Escape') hide();
};
onMounted(() => {
  window.addEventListener('scroll', scroll, true);
  window.addEventListener('resize', hide);
  window.addEventListener('keydown', escape);
});
onBeforeUnmount(() => {
  window.removeEventListener('scroll', scroll, true);
  window.removeEventListener('resize', hide);
  window.removeEventListener('keydown', escape);
});
</script>
<template>
  <Teleport to="body"
    ><Transition name="activity-tip"
      ><div
        v-if="target"
        :id="id"
        class="activity-tooltip"
        :style="position"
        role="tooltip"
      >
        <strong>{{ title }}</strong
        ><span>{{ text }}</span>
      </div></Transition
    ></Teleport
  >
</template>
<style scoped>
.activity-tooltip {
  position: fixed;
  z-index: calc(var(--z-navigation) + 1);
  width: 240px;
  padding: 12px 16px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-floating);
  pointer-events: none;
  color: var(--text);
  font-size: 12px;
}
.activity-tooltip strong,
.activity-tooltip span {
  display: block;
}
.activity-tooltip strong {
  margin-bottom: 4px;
  font-weight: 600;
}
.activity-tooltip span {
  color: var(--muted);
}
.activity-tip-enter-active,
.activity-tip-leave-active {
  transition: opacity var(--micro-duration) var(--ease-standard);
}
.activity-tip-enter-from,
.activity-tip-leave-to {
  opacity: 0;
}
</style>
