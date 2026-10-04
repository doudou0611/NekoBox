import protocol from '../../shared/protocol.json';
import type {
  LaunchSession,
  SaveSnapshot,
  TaskProgress,
  Timestamp,
} from './domain';
export const EVENTS = protocol.events;
export interface EventPayloads {
  scan_progress: TaskProgress;
  metadata_progress: TaskProgress;
  session_started: LaunchSession;
  session_ended: LaunchSession & {
    ended_at: Timestamp;
    duration_seconds: number;
    end_reason: string;
    save_backup_error: string | null;
  };
  backup_created: SaveSnapshot;
  download_progress: TaskProgress;
}
export type EventKey = keyof EventPayloads;
export interface EventEnvelope<T> {
  request_id: string;
  occurred_at: Timestamp;
  payload: T;
}
