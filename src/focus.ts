import { invoke } from '@tauri-apps/api/core';
import type { ActiveFocusSession, FocusSessionSummary, StartFocusResult } from './types';

export async function startFocusSession(taskId: string | null): Promise<StartFocusResult> {
  return invoke<StartFocusResult>('start_focus_session', { taskId });
}

export async function pauseFocusSession(sessionId: string): Promise<void> {
  return invoke<void>('pause_focus_session', { sessionId });
}

export async function resumeFocusSession(sessionId: string): Promise<void> {
  return invoke<void>('resume_focus_session', { sessionId });
}

export async function endFocusSession(sessionId: string): Promise<FocusSessionSummary> {
  return invoke<FocusSessionSummary>('end_focus_session', { sessionId });
}

export async function getActiveFocusSession(): Promise<ActiveFocusSession | null> {
  return invoke<ActiveFocusSession | null>('get_active_focus_session');
}
