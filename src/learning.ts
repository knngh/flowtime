import { invoke } from '@tauri-apps/api/core';
import type {
  EfficiencyPattern,
  PeakHoursSuggestion,
  CalibrationSummary,
} from './types';

export async function getEfficiencyPattern(
  days?: number
): Promise<EfficiencyPattern> {
  return invoke('get_efficiency_pattern', { days: days ?? null });
}

export async function calibrateEstimate(): Promise<CalibrationSummary> {
  return invoke('calibrate_estimate');
}

export async function getPeakHours(): Promise<PeakHoursSuggestion> {
  return invoke('get_peak_hours');
}

export async function getCalibrationRatio(): Promise<number> {
  return invoke('get_calibration_ratio');
}