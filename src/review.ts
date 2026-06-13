import { invoke } from '@tauri-apps/api/core';
import type { WeeklyReport, DailySummary } from './types';

export async function getWeeklyReport(weekStart: string): Promise<WeeklyReport> {
  return invoke('get_weekly_report', { weekStart });
}

export async function getDailySummary(date: string): Promise<DailySummary> {
  return invoke('get_daily_summary', { date });
}