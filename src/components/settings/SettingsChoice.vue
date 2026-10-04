<script setup lang="ts">
defineProps<{
  label: string;
  options: { value: string; label: string; description?: string }[];
  disabled?: boolean;
}>();
const model = defineModel<string>({ required: true });
</script>
<template>
  <fieldset class="preference-choice" :disabled="disabled">
    <legend>{{ label }}</legend>
    <div class="preference-options">
      <label
        v-for="option in options"
        :key="option.value"
        class="preference-option"
        :class="{ selected: model === option.value }"
      >
        <input
          v-model="model"
          type="radio"
          :value="option.value"
          :aria-label="`${label}：${option.label}`"
        />
        <span class="preference-option-dot" aria-hidden="true" />
        <span
          ><strong>{{ option.label }}</strong
          ><small v-if="option.description">{{
            option.description
          }}</small></span
        >
      </label>
    </div>
  </fieldset>
</template>
