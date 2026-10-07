<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue';
import { openAccountDialog } from '../stores/accountDialog';
import { api, desktop, errorText, notify } from '../stores/library';
import type { HikarinagiAccount } from '../types/accounts';
import type { HikarinagiReview } from '../types/hikarinagi';
const props = defineProps<{ gameId: string; remoteId: string }>();
const emit = defineEmits<{ submitted: [] }>();
const dialog = ref<HTMLDialogElement>();
const account = ref<HikarinagiAccount | null>(null);
const previous = ref<HikarinagiReview | null>(null);
const score = ref(0);
const hovering = ref(0);
const comment = ref('');
const loading = ref(false);
const submitting = ref(false);
const ready = ref(false);
const error = ref('');
let version = 0;
let opener: HTMLElement | null = null;
const signedIn = computed(
  () => account.value?.status === 'authenticated' && !!account.value.profile,
);
const editable = computed(
  () => ready.value && signedIn.value && !loading.value && !submitting.value,
);
const canSubmit = computed(
  () => editable.value && score.value >= 1 && score.value <= 10,
);
function closed() {
  // A queued close event from the prior opening must not cancel a new opening.
  if (dialog.value?.open) return;
  version++;
  hovering.value = 0;
  if (opener?.isConnected) opener.focus({ preventScroll: true });
}
async function open() {
  if (!desktop || !props.remoteId || submitting.value) return;
  opener =
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
  const owner = ++version;
  const gameId = props.gameId;
  const remoteId = props.remoteId;
  account.value = null;
  previous.value = null;
  score.value = 0;
  hovering.value = 0;
  comment.value = '';
  error.value = '';
  ready.value = false;
  loading.value = true;
  await nextTick();
  if (owner !== version) return;
  dialog.value?.showModal();
  try {
    const current = await api('hikarinagi_account', {});
    if (owner !== version) return;
    account.value = current;
    if (current.status !== 'authenticated' || !current.profile) return;
    const result = await api('get_hikarinagi_review', {
      game_id: gameId,
      account_id: current.profile.id,
    });
    if (owner !== version) return;
    if (result.remote_id !== remoteId)
      throw new Error('作品绑定已变化，请重新打开评分窗口。');
    previous.value = result.review;
    comment.value = result.review?.comment ?? '';
    const oldScore = result.review?.score;
    score.value = oldScore != null && Number.isInteger(oldScore) ? oldScore : 0;
    ready.value = true;
  } catch (cause) {
    if (owner === version) error.value = errorText(cause);
  } finally {
    if (owner === version) {
      loading.value = false;
      await nextTick();
      if (owner === version && dialog.value?.open && ready.value)
        dialog.value
          .querySelector<HTMLButtonElement>('[role="radio"][tabindex="0"]')
          ?.focus();
    }
  }
}
function keyScore(event: KeyboardEvent, index: number) {
  if (
    !editable.value ||
    ![
      'ArrowLeft',
      'ArrowRight',
      'ArrowUp',
      'ArrowDown',
      'Home',
      'End',
    ].includes(event.key)
  )
    return;
  event.preventDefault();
  score.value =
    event.key === 'Home'
      ? 1
      : event.key === 'End'
        ? 10
        : Math.max(
            1,
            Math.min(
              10,
              index + (['ArrowRight', 'ArrowUp'].includes(event.key) ? 1 : -1),
            ),
          );
  dialog.value
    ?.querySelector<HTMLButtonElement>(`[data-score="${score.value}"]`)
    ?.focus();
}
async function submit() {
  if (!canSubmit.value || !account.value?.profile) return;
  const owner = version;
  submitting.value = true;
  error.value = '';
  try {
    const response = await api('submit_hikarinagi_review', {
      game_id: props.gameId,
      remote_id: props.remoteId,
      account_id: account.value.profile.id,
      score: score.value,
      comment: comment.value,
      expected_updated_at: previous.value?.updated_at ?? null,
    });
    if (owner !== version) return;
    notify(response.cache_warning || '评分与评论已提交到 Hikarinagi。');
    emit('submitted');
    dialog.value?.close();
  } catch (cause) {
    if (owner === version) error.value = errorText(cause);
  } finally {
    submitting.value = false;
  }
}
async function goToLogin() {
  dialog.value?.close();
  await nextTick();
  openAccountDialog('hikarinagi');
}
watch(
  () => [props.gameId, props.remoteId],
  () => {
    dialog.value?.close();
    version++;
  },
);
onUnmounted(() => {
  version++;
  dialog.value?.close();
});
defineExpose({ open });
</script>
<template>
  <Teleport to="body">
    <dialog
      ref="dialog"
      class="ui-dialog hikari-review-dialog"
      aria-labelledby="hikari-review-title"
      @close="closed"
      @cancel="submitting && $event.preventDefault()"
    >
      <header>
        <div>
          <p class="eyebrow">HIKARINAGI</p>
          <h2 id="hikari-review-title">为这部作品评分</h2>
        </div>
        <button
          class="quiet-button"
          aria-label="关闭评分窗口"
          :disabled="submitting"
          @click="dialog?.close()"
        >
          关闭
        </button>
      </header>
      <p v-if="loading" role="status" class="review-hint">
        正在读取账号与已有评分…
      </p>
      <p
        v-else-if="
          account?.status === 'signed_out' || account?.status === 'expired'
        "
        role="alert"
        class="review-hint"
      >
        尚未登录 Hikarinagi{{
          account.status === 'expired' ? '或登录已过期' : ''
        }}，请先登录后再评论。
      </p>
      <p
        v-else-if="account?.status === 'offline'"
        role="alert"
        class="review-hint"
      >
        {{ account.message }}
      </p>
      <p v-else-if="signedIn" class="review-hint">
        评论账号：{{ account?.profile?.nickname || account?.profile?.username }}
      </p>
      <form @submit.prevent="submit">
        <fieldset :disabled="!editable">
          <legend>选择星数</legend>
          <div
            class="review-stars"
            role="radiogroup"
            aria-label="作品评分，1 到 10 颗星"
            @pointerleave="hovering = 0"
          >
            <button
              v-for="index in 10"
              :key="index"
              type="button"
              role="radio"
              :aria-label="`${index} 颗星`"
              :aria-checked="score === index"
              :tabindex="score === index || (!score && index === 1) ? 0 : -1"
              :data-score="index"
              :class="{ filled: index <= (hovering || score) }"
              @click="
                score = index;
                hovering = 0;
              "
              @pointerenter="hovering = index"
              @keydown="keyScore($event, index)"
            >
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path
                  d="m12 2.6 2.9 5.9 6.5.9-4.7 4.6 1.1 6.5-5.8-3.1-5.8 3.1 1.1-6.5-4.7-4.6 6.5-.9Z"
                />
              </svg>
            </button>
          </div>
          <p class="review-score" aria-live="polite">
            {{
              hovering || score
                ? `${hovering || score} / 10`
                : '点击星星选择评分'
            }}
          </p>
          <p
            v-if="
              previous &&
              !Number.isInteger(previous.score) &&
              previous.score != null
            "
            class="review-hint"
          >
            现有评分 {{ previous.score }} / 10，请选择本次提交的星数。
          </p>
          <label for="hikari-review-comment">评论</label>
          <textarea
            id="hikari-review-comment"
            v-model="comment"
            rows="6"
            maxlength="5000"
            placeholder="写下你的游玩感想（可选）"
          />
          <p class="review-count">{{ comment.length }} / 5000</p>
        </fieldset>
        <p v-if="previous && ready" class="review-hint">
          已载入你在该作品下的评分，提交会更新这条评分与评论。
        </p>
        <p class="review-hint">将发布到 Hikarinagi 安利墙。</p>
        <p v-if="error" role="alert" class="review-error">{{ error }}</p>
        <footer>
          <button
            v-if="
              account?.status === 'signed_out' || account?.status === 'expired'
            "
            type="button"
            class="secondary-button"
            @click="goToLogin"
          >
            登录 Hikarinagi 账户
          </button>
          <button
            v-else-if="!ready && !loading"
            type="button"
            class="secondary-button"
            @click="open"
          >
            重新加载
          </button>
          <button type="submit" class="primary-button" :disabled="!canSubmit">
            {{ submitting ? '正在提交…' : '评论' }}
          </button>
        </footer>
      </form>
    </dialog>
  </Teleport>
</template>
<style scoped>
.hikari-review-dialog {
  position: fixed;
  inset: 0;
  margin: auto;
  width: min(580px, calc(100vw - 32px));
  overflow: auto;
  background: var(--surface);
  color: var(--text);
}
header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  margin-bottom: 18px;
}
h2 {
  margin: 6px 0 0;
  font-size: var(--type-dialog);
}
.eyebrow {
  margin: 0;
}
fieldset {
  min-width: 0;
  margin: 20px 0 0;
  padding: 0;
  border: 0;
}
legend,
label {
  margin-bottom: 12px;
  font-size: 14px;
  font-weight: 600;
}
.review-stars {
  display: grid;
  grid-template-columns: repeat(10, minmax(0, 1fr));
  gap: 2px;
}
.review-stars button {
  display: grid;
  place-items: center;
  min-width: 0;
  min-height: 44px;
  padding: 7px 3px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--border-strong);
  touch-action: manipulation;
}
.review-stars button:hover:not(:disabled) {
  background: var(--accent-wash);
}
.review-stars svg {
  width: 100%;
  max-width: 34px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.5;
  stroke-linejoin: round;
}
.review-stars button.filled {
  color: color-mix(in srgb, var(--accent) 75%, #000);
}
.review-stars button.filled svg {
  fill: currentColor;
}
.review-score {
  margin: 12px 0 24px;
  color: var(--accent-ink, var(--accent));
  font-size: 17px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}
label {
  display: block;
}
textarea {
  display: block;
  width: 100%;
  resize: vertical;
  min-height: 120px;
  padding: 14px;
  border: 1px solid var(--border-strong);
  border-radius: 12px;
  color: var(--text);
  background: var(--surface-hover);
  font: inherit;
  line-height: 1.65;
}
textarea:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 3px;
}
.review-count {
  margin: 8px 0 0;
  color: var(--muted);
  text-align: right;
  font-size: 12px;
}
.review-hint {
  color: var(--muted);
  font-size: 13px;
  line-height: 1.7;
}
.review-error {
  color: var(--danger);
  font-size: 13px;
  line-height: 1.7;
}
footer {
  display: flex;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 12px;
  margin-top: 24px;
}
@media (max-width: 600px) {
  .hikari-review-dialog {
    padding: 20px;
  }
  .review-stars button {
    padding: 5px 1px;
  }
}
</style>
