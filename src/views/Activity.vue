<script setup lang="ts">
import ActivityDashboard from '../components/activity/ActivityDashboard.vue';
</script>
<template>
  <div class="page-content activity-page">
    <header class="page-heading activity-page-heading">
      <div>
        <p class="eyebrow">TIME WITH YOUR STORIES</p>
        <h1 tabindex="-1" data-page-heading>留在故事里的时间。</h1>
        <p class="page-lead">每一次启程，都会成为你的游玩记录。</p>
      </div>
      <span class="activity-heading-mark" aria-hidden="true"
        >A PERSONAL<br />PLAYTIME JOURNAL<span>时光有迹，故事有你。</span></span
      >
    </header>
    <ActivityDashboard />
  </div>
</template>
