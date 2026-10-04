<script setup lang="ts">
const model = defineModel<string[]>({ required: true });
defineProps<{
  label: string;
  disabled?: boolean;
  options: readonly { value: string; label: string; description?: string }[];
}>();
function toggle(value: string) {
  model.value = model.value.includes(value)
    ? model.value.filter((v) => v !== value)
    : [...model.value, value];
}
</script>
<template>
  <fieldset class="settings-checklist" :disabled="disabled">
    <legend>{{ label }}</legend>
    <label
      v-for="item in options"
      :key="item.value"
      class="settings-check-option"
      :class="{ selected: model.includes(item.value) }"
    >
      <input
        type="checkbox"
        :checked="model.includes(item.value)"
        @change="toggle(item.value)"
      />
      <span class="settings-check-mark" aria-hidden="true">{{
        model.includes(item.value) ? '✓' : ''
      }}</span>
      <span
        ><strong>{{ item.label }}</strong
        ><small v-if="item.description">{{ item.description }}</small></span
      >
    </label>
  </fieldset>
</template>
