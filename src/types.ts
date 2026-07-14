export interface Project {
  id: string;
  name: string;
  color: string;
  created_at: string;
  updated_at: string;
}

export interface Task {
  id: string;
  title: string;
  priority: 'A' | 'B' | 'C';
  estimated_duration_min: number;
  source: 'manual' | 'github' | 'linear';
  source_url: string | null;
  project_id: string | null;
  tags: string[];
  status: 'todo' | 'in_progress' | 'done' | 'deferred';
  scheduled_start: string | null;
  scheduled_end: string | null;
  actual_start: string | null;
  actual_end: string | null;
  created_at: string;
  updated_at: string;
}

/** Task data from NL parsing, before project matching */
export interface ParsedTask {
  title: string;
  priority: string;
  duration_min: number;
  project_hint: string | null;
}

// ── M3: Focus Mode ──

export interface ActiveFocusSession {
  id: string;
  task_id: string | null;
  task_title: string | null;
  start_time: string;
  status: string;
  interruption_count: number;
  elapsed_seconds: number | null;
}

export interface FocusSessionSummary {
  session_id: string;
  task_id: string | null;
  duration_seconds: number;
  interruptions_blocked: number;
  messages_auto_replied: number;
  status: string;
  interruption_count: number;
}

export interface StartFocusResult {
  session_id: string;
  peak_hours_note: string | null;
  in_peak_hours: boolean;
}

// ── M4: Behavior Tracking ──

export interface AppTimeDistribution {
  app_name: string;
  total_seconds: number;
}

export interface ProductivityStats {
  total_focus_seconds: number;
  total_tracked_seconds: number;
  app_switch_count: number;
  top_apps: AppTimeDistribution[];
  focus_sessions_count: number;
}

export const PRIORITY_LABELS: Record<Task['priority'], string> = {
  A: '高',
  B: '中',
  C: '低',
};

export const STATUS_LABELS: Record<Task['status'], string> = {
  todo: '待办',
  in_progress: '进行中',
  done: '已完成',
  deferred: '已推迟',
};

// ── M5: External Integrations ──

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

export type IntegrationSource = 'github' | 'linear' | 'feishu';

export interface IntegrationStatus {
  source: IntegrationSource;
  label: string;
  env_var: string;
  connected: boolean;
  loading: boolean;
}

// ── M6: Auto Reply ──

export interface PendingReply {
  id: string;
  original_message: string;
  reply_draft: string;
  channel: string;
  created_at: string;
  status: string; // "pending" | "sent" | "discarded"
}

// ── M7: Review Dashboard ──

export interface TimeDistributionItem {
  category: string;
  total_seconds: number;
}

export interface HighRiskTask {
  id: string;
  title: string;
  status: string;
  deferred_count: number;
  last_deferred_at: string | null;
}

export interface WeeklyReport {
  week_start: string;
  week_end: string;
  total_focus_seconds: number;
  total_tracked_seconds: number;
  tasks_done: number;
  tasks_total: number;
  completion_rate: number;
  avg_interruptions_per_day: number;
  focus_sessions_count: number;
  time_distribution: TimeDistributionItem[];
  high_risk_tasks: HighRiskTask[];
  // Previous week comparison
  prev_week_focus_seconds: number;
  prev_week_completion_rate: number;
}

export interface DailySummary {
  date: string;
  total_focus_seconds: number;
  total_tracked_seconds: number;
  tasks_done: number;
  tasks_total: number;
  completion_rate: number;
  focus_sessions_count: number;
  interruptions_blocked: number;
  time_distribution: TimeDistributionItem[];
}

// ── M8: Behavior Learning ──

export interface HourlyFocus {
  hour: number;
  total_seconds: number;
}

export interface EfficiencyPattern {
  hourly_focus: HourlyFocus[];
  peak_start_hour: number | null;
  peak_end_hour: number | null;
  avg_daily_focus_seconds: number;
  total_focus_sessions: number;
}

export interface PeakRange {
  start_hour: number;
  end_hour: number;
  avg_focus_seconds: number;
}

export interface PeakHoursSuggestion {
  peak_hours: PeakRange[];
  insight: string;
}

export interface CalibrationSummary {
  overall_ratio: number;
  sample_count: number;
  suggestion: string;
}
