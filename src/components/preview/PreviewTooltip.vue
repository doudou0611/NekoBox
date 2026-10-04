<script setup lang="ts">
defineProps<{ text: string }>();
</script>
<template>
  <span class="tooltip-wrap"
    ><slot /><span class="preview-tooltip" role="tooltip">{{
      text
    }}</span></span
  >
</template>
