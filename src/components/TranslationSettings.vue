<script setup lang="ts">
import SettingsSection from './settings/SettingsSection.vue';
import { onMounted, reactive, ref } from 'vue';
import { api, desktop, errorText, notify } from '../stores/library';
import SettingsSwitch from './settings/SettingsSwitch.vue';
import SettingsChoice from './settings/SettingsChoice.vue';
import { computed } from 'vue';
import type { TranslationSettings } from '../types/translation';
const settings = reactive<TranslationSettings>({
  enabled: false,
  languages: ['all'],
  fields: ['title', 'description', 'source_tags'],
  endpoint: '',
  model: '',
  has_api_key: false,
});
const language = computed({
  get: () => settings.languages[0] || 'all',
  set: (value: string) => {
    settings.languages = [value as 'en' | 'ja' | 'all'];
  },
});
const fieldOptions = [
  { value: 'title' as const, label: '名称' },
  { value: 'description' as const, label: '简介' },
  { value: 'source_tags' as const, label: '标签' },
];
const key = ref('');
const clearKey = ref(false);
const busy = ref(false);
const loaded = ref(false);
const error = ref('');
const testResult = ref('');
async function load() {
  if (!desktop) return;
  busy.value = true;
  error.value = '';
  try {
    Object.assign(settings, await api('get_translation_settings', {}));
    loaded.value = true;
  } catch (e) {
    error.value = errorText(e);
  } finally {
    busy.value = false;
  }
}
async function save(test = false) {
  if (!desktop || busy.value || !loaded.value) return;
  busy.value = true;
  error.value = '';
  testResult.value = '';
  try {
    Object.assign(
      settings,
      await api('save_translation_settings', {
        enabled: settings.enabled,
        languages: settings.languages,
        fields: settings.fields,
        endpoint: settings.endpoint,
        model: settings.model,
        api_key: key.value.trim() || null,
        clear_api_key: clearKey.value,
      }),
    );
    key.value = '';
    clearKey.value = false;
    if (test) {
      const result = await api('test_translation', {});
      testResult.value = result.translated_text;
      notify('翻译接口测试成功。');
    } else {
      notify('翻译设置已保存，下次刮削时生效。');
    }
  } catch (e) {
    error.value = errorText(e);
  } finally {
    busy.value = false;
  }
}
onMounted(load);
</script>
<template>
  <SettingsSection
    title="资料语言与翻译"
    description="选择遇到哪些语言时翻译，以及要翻译的资料字段。"
  >
    <form class="settings-card translation-card" @submit.prevent="save()">
      <SettingsSwitch
        v-model="settings.enabled"
        label="使用资料翻译"
        description="按所选语言与字段翻译，保留原文和手工内容。"
        :disabled="!loaded || busy"
      />
      <SettingsChoice
        v-model="language"
        label="遇到哪种语言时翻译"
        :options="[
          { value: 'en', label: '英文' },
          { value: 'ja', label: '日文' },
          { value: 'all', label: '全部外文' },
        ]"
        :disabled="!loaded || busy"
      />
      <fieldset class="preference-choice" :disabled="!loaded || busy">
        <legend>翻译范围</legend>
        <div class="preference-options">
          <label
            v-for="field in fieldOptions"
            :key="field.value"
            class="preference-option"
            :class="{ selected: settings.fields.includes(field.value) }"
            ><input
              v-model="settings.fields"
              type="checkbox"
              :value="field.value"
              :aria-label="`翻译${field.label}`"
            /><span
              class="preference-option-dot"
              aria-hidden="true"
            /><strong>{{ field.label }}</strong></label
          >
        </div>
      </fieldset>
      <p class="settings-explanation">
        译为简体中文。开发商与发行商不参与翻译；已有作品可通过下方全库更新重新翻译。
      </p>
      <div class="preference-fields">
        <label class="preference-field preference-field-full"
          >API 地址<input
            v-model="settings.endpoint"
            aria-label="API 地址"
            type="url"
            placeholder="https://你的服务地址/v1"
            :disabled="!loaded || busy"
            spellcheck="false"
            autocomplete="off"
          /><small
            >OpenAI 兼容接口，支持基础地址或完整 /chat/completions 地址</small
          ></label
        >
        <label class="preference-field"
          >模型名称<input
            v-model="settings.model"
            aria-label="模型名称"
            placeholder="服务提供的模型名称"
            :disabled="!loaded || busy"
            spellcheck="false"
            autocomplete="off"
        /></label>
        <label class="preference-field"
          >API Key<input
            v-model="key"
            aria-label="API Key"
            type="password"
            :placeholder="
              settings.has_api_key
                ? '已保存密钥，留空保留'
                : '输入密钥；本地无鉴权服务可留空'
            "
            :disabled="!loaded || busy || clearKey"
            autocomplete="new-password"
          /><small
            >密钥仅保存在本机系统凭据库，不包含在数据库导出中</small
          ></label
        >
        <SettingsSwitch
          v-if="settings.has_api_key"
          v-model="clearKey"
          class="preference-field-full"
          label="清除已保存的密钥"
          :disabled="busy"
          @update:model-value="key = ''"
        />
      </div>
      <p v-if="!desktop" class="settings-explanation">
        浏览器预览无法保存翻译配置或请求真实 API，请在桌面软件中配置。
      </p>
      <p v-if="error" class="translation-error" role="alert">
        {{ error
        }}<button
          v-if="!loaded && desktop"
          type="button"
          class="quiet-button"
          :disabled="busy"
          @click="load"
        >
          重新读取
        </button>
      </p>
      <p v-if="testResult" role="status">测试译文：{{ testResult }}</p>
      <div class="preference-actions">
        <button
          type="submit"
          class="primary-button"
          :disabled="!loaded || busy"
        >
          {{ busy ? '正在处理…' : '保存翻译设置' }}
        </button>
        <button
          type="button"
          class="secondary-button"
          :disabled="!loaded || busy || !settings.enabled"
          @click="save(true)"
        >
          保存并测试翻译
        </button>
      </div>
    </form>
  </SettingsSection>
</template>
<style scoped>
.translation-error {
  color: var(--danger);
}
</style>
