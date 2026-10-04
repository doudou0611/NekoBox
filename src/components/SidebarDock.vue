<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useMediaQuery } from '@vueuse/core';
import { useRoute } from 'vue-router';
import {
  PopoverRoot,
  PopoverTrigger,
  PopoverPortal,
  PopoverContent,
} from 'reka-ui';
import { activeOperationCount } from '../stores/operations';
import SidebarTools from './SidebarTools.vue';
import AccountAvatar from './AccountAvatar.vue';
const props = defineProps<{ expanded: boolean }>();
const emit = defineEmits<{ account: [] }>();
const wide = useMediaQuery('(min-width: 801px)');
const showTools = computed(() => props.expanded && wide.value);
const open = ref(false);
const avatarButton = ref<HTMLButtonElement>();
const route = useRoute();
let accountHandoff = false;
watch([showTools, () => route.fullPath], () => {
  open.value = false;
});
function openAccount() {
  accountHandoff = open.value;
  open.value = false;
  if (accountHandoff) avatarButton.value?.focus({ preventScroll: true });
  emit('account');
}
function restoreFocus(event: Event) {
  if (accountHandoff) {
    event.preventDefault();
    accountHandoff = false;
  }
}
</script>
<template>
  <div class="sidebar-dock">
    <Transition name="dock-view" mode="out-in">
      <div v-if="showTools" key="tools" class="sidebar-dock-expanded">
        <SidebarTools @account="openAccount" />
      </div>
      <div v-else key="avatar" class="sidebar-dock-compact">
        <PopoverRoot v-model:open="open">
          <PopoverTrigger as-child>
            <button
              ref="avatarButton"
              class="sidebar-menu-avatar"
              type="button"
              aria-label="打开快捷菜单"
              title="我的工作台"
            >
              <AccountAvatar />
              <span
                v-if="activeOperationCount"
                class="sidebar-task-dot"
                aria-label="有后台任务进行中"
              />
            </button>
          </PopoverTrigger>
          <PopoverPortal>
            <PopoverContent
              class="sidebar-quick-panel"
              data-sidebar-overlay
              aria-label="应用快捷菜单"
              side="right"
              align="end"
              :side-offset="14"
              :collision-padding="16"
              @escape-key-down="(event) => event.stopPropagation()"
              @close-auto-focus="restoreFocus"
            >
              <header><span>NEKOBOX</span><strong>我的工作台</strong></header>
              <SidebarTools
                menu
                @account="openAccount"
                @navigate="open = false"
              />
            </PopoverContent>
          </PopoverPortal>
        </PopoverRoot>
      </div>
    </Transition>
  </div>
</template>
