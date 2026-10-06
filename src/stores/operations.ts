import { computed, reactive } from 'vue';

export type OperationStatus =
  'queued' | 'running' | 'completed' | 'failed' | 'cancelled';
export interface OperationItem {
  id: string;
  title: string;
  kind: 'scrape' | 'download' | 'import' | 'sync';
  status: OperationStatus;
  progress: number | null;
  total: number | null;
  message: string;
  created_at: number;
  completed_at: number | null;
  cancel?: () => void;
  retry?: () => void;
  progress_unit?: 'bytes';
  open_details: (() => void) | null;
}

export const operations = reactive<OperationItem[]>([]);
export const activeOperationCount = computed(
  () =>
    operations.filter((item) => ['queued', 'running'].includes(item.status))
      .length,
);
export const activeOperationProgress = computed(() => {
  const active = operations.filter(
    (item) => ['queued', 'running'].includes(item.status) && item.total,
  );
  // Bytes and game counts are different units: average each task's fraction.
  return active.length
    ? Math.min(
        100,
        (active.reduce(
          (sum, item) =>
            sum + Math.min(1, Math.max(0, (item.progress ?? 0) / item.total!)),
          0,
        ) /
          active.length) *
          100,
      )
    : null;
});

let sequence = 0;
export function createOperation(
  title: string,
  kind: OperationItem['kind'],
  total: number | null = null,
  open_details: (() => void) | null = null,
): OperationItem {
  const item: OperationItem = {
    id: `operation-${Date.now()}-${++sequence}`,
    title,
    kind,
    status: 'queued',
    progress: total === 0 ? 0 : null,
    total,
    message: '排队中',
    created_at: Date.now(),
    completed_at: null,
    open_details,
  };
  operations.unshift(item);
  return item;
}
export function updateOperation(
  id: string,
  patch: Partial<
    Pick<OperationItem, 'status' | 'progress' | 'total' | 'message'>
  >,
) {
  const item = operations.find((candidate) => candidate.id === id);
  if (!item) return;
  Object.assign(item, patch);
  if (
    item.status === 'completed' ||
    item.status === 'failed' ||
    item.status === 'cancelled'
  )
    item.completed_at = Date.now();
}
export function removeOperation(id: string) {
  const index = operations.findIndex((item) => item.id === id);
  if (index >= 0) operations.splice(index, 1);
}
