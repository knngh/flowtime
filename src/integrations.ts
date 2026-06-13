import { invoke } from '@tauri-apps/api/core';

export interface ExternalTask {
  external_id: string;
  title: string;
  source: string; // "github" | "linear" | "feishu"
  url: string | null;
  priority_hint: string; // "A" | "B" | "C"
}

export interface ImportError {
  source: string;
  message: string;
}

export interface ImportResult {
  tasks: ExternalTask[];
  errors: ImportError[];
}

export async function fetchGithubIssues(): Promise<ExternalTask[]> {
  return invoke<ExternalTask[]>('fetch_github_issues');
}

export async function fetchLinearIssues(): Promise<ExternalTask[]> {
  return invoke<ExternalTask[]>('fetch_linear_issues');
}

export async function fetchFeishuEvents(): Promise<ExternalTask[]> {
  return invoke<ExternalTask[]>('fetch_feishu_events');
}

export async function importExternalTasks(sources: string[]): Promise<ImportResult> {
  return invoke<ImportResult>('import_external_tasks', { sources });
}