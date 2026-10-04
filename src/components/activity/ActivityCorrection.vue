<script setup lang="ts">
import { computed, nextTick, ref } from 'vue';
import {
  api,
  desktop,
  errorText,
  notify,
  refreshLibrary,
} from '../../stores/library';
import { correctionSeconds, durationText } from '../../services/playtime';
import type { PlaySession } from '../../types/playtime';
import PreviewIcon from '../preview/PreviewIcon.vue';
const emit = defineEmits<{ saved: [] }>();
const dialog = ref<HTMLDialogElement>(),
  session = ref<PlaySession | null>(null);
const hours = ref(0),
  minutes = ref(0),
  seconds = ref(0),
  reason = ref(''),
  confirmed = ref(false),
  saving = ref(false),
  error = ref('');
const total = computed(() =>
  correctionSeconds(hours.value, minutes.value, seconds.value),
);
const canSave = computed(
  () =>
    desktop &&
    total.value !== null &&
    reason.value.trim().length > 0 &&
    confirmed.value &&
    !saving.value,
);
let opener: HTMLElement | null = null;
async function open(value: PlaySession, event: Event) {
  if (!desktop || !value.ended_at || saving.value) return;
  opener = event.currentTarget as HTMLElement;
  session.value = value;
  hours.value = Math.floor(value.duration_seconds / 3600);
  minutes.value = Math.floor((value.duration_seconds % 3600) / 60);
  seconds.value = value.duration_seconds % 60;
  reason.value = '';
  confirmed.value = false;
  error.value = '';
  await nextTick();
  dialog.value?.showModal();
}
function close() {
  if (!saving.value) dialog.value?.close();
}
function closed() {
  opener?.focus({ preventScroll: true });
}
async function save() {
  if (!canSave.value || !session.value || total.value == null) return;
  saving.value = true;
  error.value = '';
  try {
    await api('correct_play_session', {
      session_id: session.value.id,
      expected_duration_seconds: session.value.duration_seconds,
      duration_seconds: total.value,
      reason: reason.value.trim(),
      confirmed: true,
    });
    dialog.value?.close();
    notify('游玩时长已修正。');
    emit('saved');
    try {
      await refreshLibrary();
    } catch {
      notify('时长已保存，游戏汇总刷新失败，请重试刷新。');
    }
  } catch (cause) {
    error.value = errorText(cause);
  } finally {
    saving.value = false;
  }
}
defineExpose({ open });
</script>
<template>
  <dialog
    ref="dialog"
    class="activity-correction"
    aria-labelledby="correction-title"
    @cancel.prevent="close"
    @close="closed"
  >
    <form @submit.prevent="save">
      <header>
        <div>
          <p class="activity-kicker">KEEP YOUR MEMORIES TRUE</p>
          <h2 id="correction-title">修正这段时间</h2>
        </div>
        <button
          type="button"
          class="icon-button"
          aria-label="关闭时长修正"
          :disabled="saving"
          @click="close"
        >
          <PreviewIcon name="close" />
        </button>
      </header>
      <p class="correction-game">{{ session?.game_title }}</p>
      <p class="activity-note">
        原时长：{{ durationText(session?.duration_seconds ?? 0) }}
      </p>
      <div class="correction-fields">
        <label
          >小时<input
            v-model.number="hours"
            type="number"
            min="0"
            max="87600"
            step="1"
            required
            :disabled="saving" /></label
        ><label
          >分钟<input
            v-model.number="minutes"
            type="number"
            min="0"
            max="59"
            step="1"
            required
            :disabled="saving" /></label
        ><label
          >秒<input
            v-model.number="seconds"
            type="number"
            min="0"
            max="59"
            step="1"
            required
            :disabled="saving"
        /></label>
      </div>
      <label class="correction-reason"
        >修正原因<input
          v-model="reason"
          maxlength="500"
          required
          placeholder="写下需要调整这次时长的原因"
          :disabled="saving"
      /></label>
      <label class="correction-confirm"
        ><input
          v-model="confirmed"
          type="checkbox"
          :disabled="saving"
        />确认将本次会话更新为
        {{ total == null ? '有效时长' : durationText(total) }}</label
      >
      <p v-if="error" class="activity-error" role="alert">
        {{ error
        }}<button
          type="button"
          class="quiet-button"
          :disabled="saving"
          @click="
            close();
            emit('saved');
          "
        >
          关闭并刷新记录
        </button>
      </p>
      <footer>
        <button
          type="button"
          class="quiet-button"
          :disabled="saving"
          @click="close"
        >
          取消</button
        ><button class="primary-button" type="submit" :disabled="!canSave">
          {{ saving ? '保存中…' : '保存修正' }}
        </button>
      </footer>
    </form>
  </dialog>
</template>
<style scoped>
.activity-correction {
  width: min(520px, calc(100vw - 40px));
  max-height: calc(100vh - 48px);
  padding: var(--space-24);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-xl);
  background: var(--surface);
  color: var(--text);
  box-shadow: var(--shadow-modal);
}
.activity-correction::backdrop {
  background: color-mix(in srgb, var(--background) 70%, transparent);
}
.activity-correction[open] {
  animation: correction-enter var(--feedback-duration) var(--ease-standard) both;
}
.activity-correction header,
.activity-correction footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
}
.activity-correction h2 {
  font-size: 26px;
}
.correction-game {
  margin-top: 24px;
  font-size: 17px;
}
.correction-fields {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
  margin: 24px 0;
}
.correction-fields label,
.correction-reason {
  display: grid;
  gap: 8px;
  color: var(--muted);
  font-size: 12px;
}
.activity-correction input:not([type='checkbox']) {
  width: 100%;
  min-width: 0;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface-hover);
  color: var(--text);
  padding: 10px 12px;
}
.correction-confirm {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 20px 0;
  font-size: 12px;
}
.correction-confirm input {
  accent-color: var(--accent-ink, var(--text));
}
.activity-correction footer {
  justify-content: flex-end;
  margin-top: 24px;
}
.activity-correction .activity-kicker {
  color: var(--muted);
  font-size: 9px;
  letter-spacing: 0.14em;
}
.activity-correction .activity-note {
  color: var(--muted);
  font-size: 12px;
}
.activity-correction .activity-error {
  color: var(--danger);
  font-size: 13px;
}
@keyframes correction-enter {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
:root[data-motion='reduced'] .activity-correction[open] {
  animation: none;
}
</style>
