<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import {
  hikariField,
  chooseHikariFolder,
  confirmHikariFolder,
} from '../stores/hikariField';
import { errorText } from '../stores/library';
import PreviewIcon from './preview/PreviewIcon.vue';
const dialog = ref<HTMLDialogElement>();
const destination = computed(() => {
  const p = hikariField.folder_parent.replace(/[\\/]+$/, '');
  return /(?:^|[\\/])HikariFieldGames$/i.test(p)
    ? p
    : `${p}${p.includes('\\') ? '\\' : '/'}HikariFieldGames`;
});
const opener =
  document.activeElement instanceof HTMLElement ? document.activeElement : null;
function close() {
  if (!hikariField.folder_busy) {
    hikariField.folder_game = '';
    opener?.focus();
  }
}
async function choose() {
  try {
    const p = await chooseHikariFolder();
    if (p) hikariField.folder_parent = p;
  } catch (e) {
    hikariField.folder_error = errorText(e);
  }
}
onMounted(() => dialog.value?.showModal());
</script>
<template>
  <dialog
    ref="dialog"
    class="hf-folder-dialog"
    aria-labelledby="hf-folder-title"
    @cancel.prevent="close"
  >
    <div class="hf-folder-heading">
      <span><PreviewIcon name="folder" :size="28" /></span
      ><button
        class="icon-button"
        aria-label="关闭下载目录窗口"
        :disabled="hikariField.folder_busy"
        @click="close"
      >
        <PreviewIcon name="close" />
      </button>
    </div>
    <p class="eyebrow">A PLACE FOR YOUR STORIES</p>
    <h2 id="hf-folder-title">为故事选一个家</h2>
    <p class="hf-folder-copy">
      选择 HIKARI FIELD 游戏的存放位置。所有下载的游戏都会放进
      <strong>HikariFieldGames</strong> 文件夹，只需设置一次。
    </p>
    <button
      class="hf-folder-picker"
      :disabled="hikariField.folder_busy"
      @click="choose"
    >
      <PreviewIcon name="folder" :size="20" /><span>{{
        hikariField.folder_parent ? '更换存放位置' : '选择存放位置'
      }}</span
      ><PreviewIcon name="arrow" :size="18" />
    </button>
    <Transition name="hf-path"
      ><div v-if="hikariField.folder_parent" class="hf-destination">
        <small>游戏将保存在</small>
        <p>{{ destination }}</p>
      </div></Transition
    >
    <p v-if="hikariField.folder_error" role="alert">
      {{ hikariField.folder_error }}
    </p>
    <small class="hf-folder-note">之后可在设置中查看或更改位置。</small>
    <footer>
      <button
        class="quiet-button"
        :disabled="hikariField.folder_busy"
        @click="close"
      >
        稍后再说</button
      ><button
        class="primary-button"
        :disabled="!hikariField.folder_parent || hikariField.folder_busy"
        @click="confirmHikariFolder"
      >
        {{ hikariField.folder_busy ? '正在准备…' : '保存并下载游戏' }}
      </button>
    </footer>
  </dialog>
</template>
<style scoped>
.hf-folder-dialog {
  margin: auto;
  width: min(480px, calc(100vw - 32px));
  max-height: calc(100dvh - 48px);
  overflow: auto;
  box-sizing: border-box;
  padding: 30px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  color: var(--text);
  background: var(--surface);
  box-shadow: var(--shadow-modal);
}
.hf-folder-dialog::backdrop {
  background: rgb(32 27 45 / 36%);
  backdrop-filter: blur(8px);
}
.hf-folder-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 22px;
}
.hf-folder-heading > span {
  display: grid;
  place-items: center;
  width: 56px;
  height: 56px;
  border-radius: 18px;
  background: var(--accent-wash);
  color: var(--accent);
}
h2 {
  font-size: 26px;
  margin: 8px 0 14px;
}
.hf-folder-copy {
  color: var(--muted);
  font-size: 13px;
  line-height: 1.9;
}
.hf-folder-copy strong {
  color: var(--text);
  font-weight: 500;
}
.hf-folder-picker {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  padding: 17px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  color: var(--accent-ink, var(--accent));
  background: var(--accent-wash);
  cursor: pointer;
}
.hf-folder-picker span {
  flex: 1;
  text-align: left;
}
.hf-destination {
  margin-top: 16px;
  padding: 14px 16px;
  border-radius: var(--radius-sm);
  background: var(--surface-hover);
}
.hf-destination small,
.hf-folder-note {
  color: var(--muted);
  font-size: 11px;
}
.hf-destination p {
  font-size: 13px;
  overflow-wrap: anywhere;
  margin: 6px 0 0;
  line-height: 1.7;
}
.hf-folder-note {
  display: block;
  margin-top: 16px;
}
footer {
  display: flex;
  gap: 10px;
  justify-content: flex-end;
  margin-top: 26px;
}
.hf-path-enter-active,
.hf-path-leave-active {
  transition:
    opacity 180ms,
    transform 180ms;
}
.hf-path-enter-from,
.hf-path-leave-to {
  opacity: 0;
  transform: translateY(6px);
}
@media (prefers-reduced-motion: reduce) {
  * {
    transition: none !important;
  }
}
:global(:root[data-motion='reduced'] .hf-folder-dialog *) {
  transition: none !important;
}
</style>
