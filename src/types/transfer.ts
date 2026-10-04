export interface DatabaseExportResult {
  path: string;
  sha256: string;
  size_bytes: number;
}
export interface DatabaseImportPreview {
  confirmation_token: string;
  sha256: string;
  expires_at: string;
  counts: {
    games: number;
    installations: number;
    collections: number;
    sessions: number;
  };
}
export interface DatabaseTransferStatus {
  pending_restart: boolean;
  last_import_error: string | null;
}
