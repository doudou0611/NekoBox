<script setup lang="ts">
import { ref } from 'vue';
import PreviewIcon from './preview/PreviewIcon.vue';
import { formatBytes } from '../stores/hikariField';
import {
  PopoverRoot,
  PopoverTrigger,
  PopoverPortal,
  PopoverContent,
} from 'reka-ui';
defineProps<{ menu?: boolean }>();
import {
  activeOperationCount,
  activeOperationProgress,
  operations,
  removeOperation,
  type OperationItem,
} from '../stores/operations';

const open = ref(false);
function statusLabel(status: OperationItem['status']) {
  return {
    queued: '排队中',
    running: '进行中',
    cancelled: '已取消',
    completed: '已完成',
    failed: '失败',
  }[status];
}
</script>
<template>
  <div class="operation-center">
    <PopoverRoot v-model:open="open">
      <PopoverTrigger as-child>
        <button
          class="icon-button operation-trigger sidebar-tool"
          type="button"
          aria-label="打开后台任务"
          title="后台任务"
        >
          <PreviewIcon name="download" :size="20" />
          <span v-if="menu" class="sidebar-tool-copy"
            >后台任务<small>{{
              activeOperationCount
                ? `${activeOperationCount} 项进行中`
                : '查看任务进度'
            }}</small></span
          >
          <span
            v-if="activeOperationProgress !== null"
            class="operation-trigger-progress"
            aria-hidden="true"
          >
            <span :style="{ width: `${activeOperationProgress}%` }"></span>
          </span>
          <span v-if="activeOperationCount" class="operation-count">{{
            activeOperationCount
          }}</span>
        </button>
      </PopoverTrigger>
      <PopoverPortal>
        <PopoverContent
          class="operation-popover"
          data-sidebar-overlay
          aria-label="后台任务列表"
          side="top"
          align="end"
          :side-offset="14"
          :collision-padding="16"
          @escape-key-down="(event) => event.stopPropagation()"
        >
          <header>
            <strong>后台任务</strong>
            <span>{{
              activeOperationCount
                ? `${activeOperationCount} 项进行中`
                : '暂无进行中任务'
            }}</span>
          </header>
          <p v-if="!operations.length" class="operation-empty">
            暂无任务记录。
          </p>
          <ul v-else>
            <li
              v-for="item in operations"
              :key="item.id"
              :data-status="item.status"
            >
              <div class="operation-line">
                <strong>{{ item.title }}</strong>
                <span>{{ statusLabel(item.status) }}</span>
              </div>
              <p>{{ item.message }}</p>
              <small v-if="item.total"
                >{{
                  item.progress_unit === 'bytes'
                    ? formatBytes(item.progress ?? 0)
                    : (item.progress ?? 0)
                }}
                /
                {{
                  item.progress_unit === 'bytes'
                    ? formatBytes(item.total)
                    : `${item.total} 项已处理`
                }}</small
              >
              <div
                v-if="item.total"
                class="operation-progress"
                role="progressbar"
                :aria-label="item.title"
                :aria-valuemin="0"
                :aria-valuemax="item.total"
                :aria-valuenow="item.progress ?? 0"
              >
                <span
                  :style="{
                    width: `${Math.min(100, ((item.progress ?? 0) / item.total) * 100)}%`,
                  }"
                ></span>
              </div>
              <div class="operation-actions">
                <button
                  v-if="item.cancel"
                  class="quiet-button"
                  type="button"
                  @click="item.cancel()"
                >
                  {{
                    item.kind === 'download' ? '取消下载' : '停止任务'
                  }}</button
                ><button
                  v-if="item.retry"
                  class="quiet-button"
                  type="button"
                  @click="item.retry()"
                >
                  {{ item.kind === 'download' ? '继续下载' : '重试未完成项' }}
                </button>
              </div>
              <button
                v-if="item.open_details"
                type="button"
                class="quiet-button"
                @click="
                  item.open_details();
                  open = false;
                "
              >
                查看刮削结果
              </button>
              <button
                v-if="
                  ['completed', 'failed', 'cancelled'].includes(item.status)
                "
                class="operation-remove"
                type="button"
                :aria-label="`擦除任务：${item.title}`"
                @click="removeOperation(item.id)"
              >
                <PreviewIcon name="close" :size="14" />
              </button>
            </li>
          </ul>
        </PopoverContent>
      </PopoverPortal>
    </PopoverRoot>
  </div>
</template>
<style scoped>
.operation-trigger-progress {
  position: absolute;
  left: 4px;
  right: 4px;
  bottom: 2px;
  height: 3px;
  overflow: hidden;
  border-radius: var(--radius-pill);
  background: var(--accent-wash);
}
.operation-trigger-progress > span {
  display: block;
  height: 100%;
  background: var(--accent);
  transition: width var(--feedback-duration);
}
</style>
