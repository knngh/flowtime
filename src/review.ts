import { invoke } from '@tauri-apps/api/core';
import type { WeeklyReport, DailySummary } from './types';

export async function getWeeklyReport(weekStart: string): Promise<WeeklyReport> {
  return invoke('get_weekly_report', { weekStart });
}

export async function getDailySummary(date: string): Promise<DailySummary> {
  return invoke('get_daily_summary', { date });
}

// ── Task deferral (P1-2) ──

export async function deferTask(taskId: string): Promise<void> {
  return invoke<void>('defer_task', { taskId });
}

// ── Custom app category rules (P3-3) ──

export async function getAppCategories(): Promise<Record<string, string>> {
  return invoke<Record<string, string>>('get_app_categories');
}

export async function setAppCategory(app: string, category: string): Promise<void> {
  return invoke<void>('set_app_category', { app, category });
}

export async function deleteAppCategory(app: string): Promise<void> {
  return invoke<void>('delete_app_category', { app });
}