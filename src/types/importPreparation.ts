export interface ImportPreparation {
  preparation_id: string;
  title: string;
  subtitle: string | null;
  cover_path: string;
  provider: string;
  remote_id: string;
  translation_message: string | null;
  supplementation_message?: string | null;
}
export interface PrepareImportRequest {
  manual?: boolean;
  single_source?: boolean;
  title_hint?: string;
  batch_id?: string;
  directory: string;
  provider: string;
  remote_id: string;
}
export interface CommitImportRequest {
  directory: string;
  title: string;
  executable_path: string | null;
  preparation_id: string | null;
}
