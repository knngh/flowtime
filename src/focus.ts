import { invoke } from '@tauri-apps/api/core';
import type { ActiveFocusSession, FocusSessionSummary } from './types';

export async function startFocusSession(taskId: string | null): Promise<string> {
  return invoke<string>('start_focus_session', { taskId });
}

export async function endFocusSession(sessionId: string): Promise<FocusSessionSummary> {
  return invoke<FocusSessionSummary>('end_focus_session', { sessionId });
}

export async function getActiveFocusSession(): Promise<ActiveFocusSession | null> {
  return invoke<ActiveFocusSession | null>('get_active_focus_session');
}
