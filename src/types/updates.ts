export interface AppUpdateStatus {
  phase:
    | 'idle'
    | 'checking'
    | 'current'
    | 'available'
    | 'downloading'
    | 'ready'
    | 'installing'
    | 'error';
  current_version: string;
  architecture: 'x64' | 'arm64' | 'unsupported';
  installation: 'installer' | 'portable' | 'unsupported';
  version: string | null;
  notes: string;
  release_url: string;
  download_url: string | null;
  signed: boolean;
  downloaded: number;
  total: number | null;
  message: string;
}
