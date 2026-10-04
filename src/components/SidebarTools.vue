<script setup lang="ts">
import { useRoute } from 'vue-router';
import { preview } from '../stores/library';
import PreviewIcon from './preview/PreviewIcon.vue';
import AccountAvatar from './AccountAvatar.vue';
import OperationCenter from './OperationCenter.vue';
defineProps<{ menu?: boolean }>();
const emit = defineEmits<{ account: []; navigate: [] }>();
const route = useRoute();
</script>
<template>
  <div class="sidebar-tools" :class="{ 'is-menu': menu }" aria-label="应用工具">
    <button
      class="sidebar-tool icon-button"
      type="button"
      aria-label="切换主题"
      :title="preview.theme === 'dark' ? '切换为浅色主题' : '切换为深色主题'"
      @click="preview.theme = preview.theme === 'dark' ? 'light' : 'dark'"
    >
      <PreviewIcon
        :name="preview.theme === 'dark' ? 'sun' : 'moon'"
        :size="20"
      />
      <span v-if="menu" class="sidebar-tool-copy"
        >切换主题<small>{{
          preview.theme === 'dark' ? '浅色展厅' : '深色展厅'
        }}</small></span
      >
    </button>
    <RouterLink
      :to="{ name: 'settings' }"
      class="sidebar-tool icon-button sidebar-settings-button"
      aria-label="设置"
      title="设置"
      :class="{ 'is-selected': route.name === 'settings' }"
      @click="emit('navigate')"
    >
      <PreviewIcon name="tune" :size="20" /><span
        v-if="menu"
        class="sidebar-tool-copy"
        >设置<small>偏好与资料</small></span
      >
    </RouterLink>
    <OperationCenter :menu="menu" />
    <button
      class="sidebar-tool icon-button sidebar-account-button"
      type="button"
      aria-label="打开账户与同步"
      title="账户与同步"
      @click="emit('account')"
    >
      <AccountAvatar /><span v-if="menu" class="sidebar-tool-copy"
        >账户与同步<small>连接我的账户</small></span
      >
    </button>
  </div>
</template>
