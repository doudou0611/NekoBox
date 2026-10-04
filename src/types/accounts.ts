import type { BangumiAccount } from './local';
export type HikarinagiAccount = BangumiAccount;
export interface BeginHikarinagiLogin {
  flow_id: string;
  authorization_url: string;
  expires_in_seconds: number;
}
export interface HikarinagiLoginFlow {
  flow_id: string;
  status: 'pending' | 'completed' | 'failed' | 'cancelled';
  message: string;
  account: HikarinagiAccount | null;
}
export interface VndbSettings {
  has_api_token: boolean;
}
export interface VndbConnection {
  username: string;
  permissions: string[];
}
export interface AccountSyncReport {
  processed: number;
  synced: number;
  skipped: number;
  failed: number;
  failures: { title: string; message: string }[];
  message: string;
}
