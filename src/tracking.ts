import { invoke } from '@tauri-apps/api/core';
import type { AppTimeDistribution, ProductivityStats } from './types';

export async function trackWindowActivity(appName: string, windowTitle: string): Promise<void> {
  return invoke('track_window_activity', { appName, windowTitle });
}

export async function getDailyTimeDistribution(date?: string): Promise<AppTimeDistribution[]> {
  return invoke<AppTimeDistribution[]>('get_daily_time_distribution', { date: date || null });
}

export async function getProductivityStats(date?: string): Promise<ProductivityStats> {
  return invoke<ProductivityStats>('get_productivity_stats', { date: date || null });
}

export async function getFrontmostApp(): Promise<string> {
  return invoke<string>('get_frontmost_app');
}