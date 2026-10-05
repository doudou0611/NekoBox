import { reactive } from 'vue';
export type AccountProvider = 'bangumi' | 'hikarinagi' | 'hikarifield';
export const accountDialog = reactive({
  open: false,
  provider: 'bangumi' as AccountProvider,
});
export function openAccountDialog(provider: AccountProvider = 'bangumi') {
  accountDialog.provider = provider;
  accountDialog.open = true;
}
