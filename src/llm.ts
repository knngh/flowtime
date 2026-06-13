import { invoke } from '@tauri-apps/api/core';

export interface ParsedTask {
  title: string;
  priority: string; // "A" | "B" | "C"
  duration_min: number;
  project_hint: string | null;
}

export interface ScheduleTaskInput {
  id: string;
  title: string;
  priority: string;
  estimated_duration_min: number;
  status: string;
}

export async function parseNaturalLanguage(input: string): Promise<ParsedTask> {
  return invoke<ParsedTask>('parse_natural_language', { input });
}

export async function suggestSchedule(tasks: ScheduleTaskInput[]): Promise<string[]> {
  const tasksJson = JSON.stringify(tasks);
  return invoke<string[]>('suggest_schedule', { tasksJson });
}