<script setup lang="ts">
import PreviewIcon from '../preview/PreviewIcon.vue';
import './account.css';
defineProps<{
  title: string;
  eyebrow: string;
  icon: string;
  titleId?: string;
  profile?: { name: string; detail: string; avatar?: string | null };
  connected?: boolean;
}>();
defineEmits<{ avatarError: [] }>();
</script>
<template>
  <div class="provider-account">
    <header class="provider-intro">
      <span class="provider-emblem"
        ><PreviewIcon :name="icon" :size="26"
      /></span>
      <div>
        <p>{{ eyebrow }}</p>
        <h3 :id="titleId">{{ title }}</h3>
      </div>
    </header>
    <div v-if="profile" class="provider-profile">
      <img
        v-if="profile.avatar"
        class="provider-avatar"
        :src="profile.avatar"
        :alt="`${title} 头像`"
        @error="$emit('avatarError')"
      />
      <span v-else class="provider-avatar" aria-hidden="true">{{
        profile.name.slice(0, 1) || title.slice(0, 1)
      }}</span>
      <div class="provider-identity">
        <strong>{{ profile.name }}</strong>
        <small>{{ profile.detail }}</small>
      </div>
      <span
        class="provider-connection"
        :aria-label="connected ? '账户已连接' : '需要重新授权'"
        role="img"
      >
        <PreviewIcon :name="connected ? 'check' : 'warning'" :size="19" />
      </span>
    </div>
    <slot />
  </div>
</template>
