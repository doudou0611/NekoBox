<script setup lang="ts">
defineProps<{ label: string; description?: string; disabled?: boolean }>();
const model = defineModel<boolean>({ required: true });
</script>
<template>
  <label class="preference-switch" :class="{ disabled }">
    <span class="preference-switch-copy"
      ><strong>{{ label }}</strong
      ><small v-if="description">{{ description }}</small></span
    >
    <span class="preference-switch-control">
      <input
        v-model="model"
        type="checkbox"
        :disabled="disabled"
        :aria-label="label"
      />
      <span class="preference-switch-track" aria-hidden="true"><span /></span>
    </span>
  </label>
</template>
