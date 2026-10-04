<script setup lang="ts">
defineProps<{ title: string; description: string }>();
</script>
<template>
  <section class="settings-section" :aria-label="title">
    <header class="settings-intro">
      <h3>{{ title }}</h3>
      <p>{{ description }}</p>
    </header>
    <div class="settings-panel">
      <slot />
    </div>
  </section>
</template>
