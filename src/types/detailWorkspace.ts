export interface MetadataValues {
  title: string;
  cover_path: string | null;
  developer: string | null;
  source_rating: number | null;
  release_date: string | null;
  description: string | null;
}
export type MetadataChanges = Partial<MetadataValues>;
export interface RunningProcess {
  pid: number;
  created_at_ticks: string;
  name: string;
  path: string;
}
