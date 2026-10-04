<script setup lang="ts">
import { computed } from 'vue';
import { bangumi } from '../stores/bangumi';
import { hikariAccount } from '../stores/hikarinagiAccount';
const account = computed(() =>
  bangumi.account.profile ? bangumi : hikariAccount,
);
</script>
<template>
  <span class="account-avatar" aria-hidden="true">
    <img
      v-if="account.account.profile?.avatar_url && !account.avatar_failed"
      :src="account.account.profile.avatar_url"
      alt=""
      @error="account.avatar_failed = true"
    />
    <span v-else>{{
      account.account.profile?.nickname?.slice(0, 1) || 'G'
    }}</span>
  </span>
</template>
