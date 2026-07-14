import { useEffect, useState, useCallback, useRef } from 'react';
import type { Project, Task, ParsedTask, ActiveFocusSession, FocusSessionSummary, StartFocusResult, ExternalTask, PendingReply, PeakHoursSuggestion } from './types';
import { getProjects, createProject, renameProject, deleteProject, getTasks, createTask, updateTask } from './db';
import { parseNaturalLanguage, suggestSchedule } from './llm';
import { startFocusSession, endFocusSession, getActiveFocusSession, pauseFocusSession, resumeFocusSession } from './focus';
import { trackWindowActivity, getFrontmostApp } from './tracking';
import { getPendingReplies, generateAutoReply } from './auto_reply';
import { getPeakHours, calibrateEstimate, getCalibrationRatio } from './learning';
import { register, unregister } from '@tauri-apps/plugin-global-shortcut';
import { isPermissionGranted, requestPermission } from '@tauri-apps/plugin-notification';
import { getCurrentWindow } from '@tauri-apps/api/window';
import Sidebar from './components/Sidebar';
import Timeline from './components/Timeline';
import TaskModal from './components/TaskModal';
import FocusMode from './components/FocusMode';
import IntegrationsPanel from './components/IntegrationsPanel';
import AutoReplyPanel from './components/AutoReplyPanel';
import WeeklyReport from './components/WeeklyReport';
import SettingsModal from './components/SettingsModal';

export default function App() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [editingTask, setEditingTask] = useState<Task | null>(null);
  const [showNewProject, setShowNewProject] = useState(false);

  // M2: NL parsing state
  const [parsingTask, setParsingTask] = useState<ParsedTask | null>(null);
  const [isParsing, setIsParsing] = useState(false);
  const [parseError, setParseError] = useState<string | null>(null);

  // M2: AI reorder state
  const [orderedTaskIds, setOrderedTaskIds] = useState<string[] | null>(null);
  const [isReordering, setIsReordering] = useState(false);

  // M3: Focus mode state
  const [activeFocus, setActiveFocus] = useState<ActiveFocusSession | null>(null);
  const [focusSummary, setFocusSummary] = useState<FocusSessionSummary | null>(null);
  const [peakNote, setPeakNote] = useState<string | null>(null);
  const activeFocusRef = useRef<ActiveFocusSession | null>(null);
  activeFocusRef.current = activeFocus;

  // M4: Tracking ref
  const trackingRef = useRef<ReturnType<typeof setInterval>>(null);

  // M5: External integrations state
  const [showIntegrations, setShowIntegrations] = useState(false);
  const [importTasks, setImportTasks] = useState<ExternalTask[]>([]);
  const [selectedImportIds, setSelectedImportIds] = useState<Set<string>>(new Set());
  const [showImportPreview, setShowImportPreview] = useState(false);

  // M6: Auto reply state
  const [pendingReplies, setPendingReplies] = useState<PendingReply[]>([]);
  const [showAutoReply, setShowAutoReply] = useState(false);

  // M7: Review dashboard state
  const [showReview, setShowReview] = useState(false);

  // M8: Behavior learning state
  const [peakHours, setPeakHours] = useState<PeakHoursSuggestion | null>(null);
  const [calibrationRatio, setCalibrationRatio] = useState<number>(1.0);

  // M9: Theme + settings (P3-3)
  const [theme, setTheme] = useState<'light' | 'dark'>(
    () => (localStorage.getItem('flowtime-theme') === 'dark' ? 'dark' : 'light'),
  );
  const [showSettings, setShowSettings] = useState(false);

  useEffect(() => {
    document.documentElement.classList.toggle('dark', theme === 'dark');
    localStorage.setItem('flowtime-theme', theme);
  }, [theme]);

  const loadData = useCallback(async (pid: string | null) => {
    const projList = await getProjects();
    setProjects(projList);
    const taskList = await getTasks(pid ?? undefined);
    setTasks(taskList);
  }, []);

  useEffect(() => {
    loadData(null);
    // Restore active focus session on app start
    getActiveFocusSession().then((s) => {
      if (s) setActiveFocus(s);
    });
  }, [loadData]);

  useEffect(() => {
    getTasks(activeProjectId ?? undefined).then((list) => {
      setTasks(list);
      setOrderedTaskIds(null);
    });
  }, [activeProjectId]);

  // ── M4: Tracking timer (every 30s) ──
  useEffect(() => {
    const tick = async () => {
      try {
        const appName = await getFrontmostApp();
        await trackWindowActivity(appName, '');
      } catch {
        // Silently ignore tracking errors
      }
    };

    trackingRef.current = setInterval(tick, 30000);
    return () => {
      if (trackingRef.current) clearInterval(trackingRef.current);
    };
  }, []);

  // ── M8: Load behavior learning data on startup ──
  useEffect(() => {
    getPeakHours().then(setPeakHours).catch(() => {});
    getCalibrationRatio().then(setCalibrationRatio).catch(() => {});
  }, []);

  // ── P0-4: Request notification permission once on startup ──
  useEffect(() => {
    (async () => {
      try {
        const granted = await isPermissionGranted();
        if (!granted) await requestPermission();
      } catch {
        // Notifications may be unavailable; ignore.
      }
    })();
  }, []);

  // ── P0-3: Global shortcuts (registered from the frontend, version-stable) ──
  useEffect(() => {
    const f = 'CmdOrCtrl+Shift+F';
    const o = 'CmdOrCtrl+Shift+O';
    (async () => {
      try {
        await register(f, () => {
          // Start (or refocus) a focus session; ignore if one is already active.
          if (activeFocusRef.current) return;
          handleQuickStartFocus();
        });
      } catch (e) {
        console.warn('Failed to register focus shortcut:', e);
      }
      try {
        await register(o, () => {
          getCurrentWindow().show().catch(() => {});
          getCurrentWindow().setFocus().catch(() => {});
        });
      } catch (e) {
        console.warn('Failed to register window shortcut:', e);
      }
    })();
    return () => {
      unregister(f).catch(() => {});
      unregister(o).catch(() => {});
    };
  }, []);

  // ── P0-1: Peak-hours linkage note auto-dismiss ──
  useEffect(() => {
    if (!peakNote) return;
    const t = setTimeout(() => setPeakNote(null), 6000);
    return () => clearTimeout(t);
  }, [peakNote]);

  // ── Project handlers ──

  const handleProjectCreated = async (name: string) => {
    await createProject(name);
    setShowNewProject(false);
    const projList = await getProjects();
    setProjects(projList);
  };

  const handleProjectRenamed = async (id: string, name: string) => {
    await renameProject(id, name);
    const projList = await getProjects();
    setProjects(projList);
  };

  const handleProjectDeleted = async (id: string) => {
    await deleteProject(id);
    if (activeProjectId === id) setActiveProjectId(null);
    const projList = await getProjects();
    setProjects(projList);
  };

  const refreshTasks = () => {
    getTasks(activeProjectId ?? undefined).then((list) => {
      setTasks(list);
      setOrderedTaskIds(null);
    });
  };

  // ── M2: NL parsing handler ──

  const handleNlSubmit = async (input: string) => {
    setIsParsing(true);
    setParseError(null);
    try {
      const parsed = await parseNaturalLanguage(input);
      setParsingTask(parsed);
    } catch (err) {
      setParseError(String(err));
    } finally {
      setIsParsing(false);
    }
  };

  // ── M2: NL-confirmed create ──

  const handleNlCreate = async (data: {
    title: string;
    priority: string;
    estimatedDurationMin: number;
    projectId: string | null;
  }) => {
    const safePriority = (['A', 'B', 'C'].includes(data.priority) ? data.priority : 'B') as Task['priority'];
    await createTask(data.title, data.projectId, safePriority, data.estimatedDurationMin);
    setParsingTask(null);
    setParseError(null);
    refreshTasks();
  };

  // ── M2: AI reorder handler ──

  const handleAiReorder = async () => {
    if (tasks.length === 0) return;
    setIsReordering(true);
    try {
      const input = tasks
        .filter((t) => t.status === 'todo' || t.status === 'in_progress')
        .map((t) => ({
          id: t.id,
          title: t.title,
          priority: t.priority,
          estimated_duration_min: t.estimated_duration_min,
          status: t.status,
        }));
      if (input.length === 0) {
        setOrderedTaskIds(null);
        return;
      }
      const ids = await suggestSchedule(input);
      setOrderedTaskIds(ids);
    } catch (err) {
      console.error('AI reorder failed:', err);
    } finally {
      setIsReordering(false);
    }
  };

  // ── M3: Focus handlers ──

  const applyFocusResult = (res: StartFocusResult, taskId: string | null, taskTitle: string | null) => {
    const now = new Date().toISOString();
    setActiveFocus({
      id: res.session_id,
      task_id: taskId,
      task_title: taskTitle,
      start_time: now,
      status: 'active',
      interruption_count: 0,
      elapsed_seconds: 0,
    });
    setPeakNote(res.peak_hours_note);
  };

  const handleStartFocus = async (taskId: string, taskTitle: string) => {
    try {
      const res = await startFocusSession(taskId);
      applyFocusResult(res, taskId, taskTitle);
    } catch (err) {
      console.error('Failed to start focus:', err);
    }
  };

  // P0-3: keyboard-shortcut entry — focus without binding a specific task.
  const handleQuickStartFocus = async () => {
    try {
      const res = await startFocusSession(null);
      applyFocusResult(res, null, null);
    } catch (err) {
      console.error('Quick-start focus failed:', err);
    }
  };

  const handlePauseFocus = async () => {
    if (!activeFocus) return;
    try {
      await pauseFocusSession(activeFocus.id);
      setActiveFocus((f) => (f ? { ...f, status: 'paused' } : f));
    } catch (err) {
      console.error('Failed to pause focus:', err);
    }
  };

  const handleResumeFocus = async () => {
    if (!activeFocus) return;
    try {
      await resumeFocusSession(activeFocus.id);
      setActiveFocus((f) => (f ? { ...f, status: 'active' } : f));
    } catch (err) {
      console.error('Failed to resume focus:', err);
    }
  };

  const handleEndFocus = async () => {
    if (!activeFocus) return;
    try {
      const result = await endFocusSession(activeFocus.id);
      setActiveFocus(null);
      setFocusSummary(result);
    } catch (err) {
      setActiveFocus(null);
      console.error('Failed to end focus:', err);
    }
  };

  const handleFocusSummaryClose = async (markDone: boolean) => {
    if (markDone && focusSummary?.task_id) {
      await updateTask(focusSummary.task_id, { status: 'done' });
      refreshTasks();
    }
    setFocusSummary(null);
    // M8: recalibrate after each completed focus session
    try {
      await calibrateEstimate();
      const [ph, cr] = await Promise.all([getPeakHours(), getCalibrationRatio()]);
      setPeakHours(ph);
      setCalibrationRatio(cr);
    } catch {
      // silently ignore
    }
  };

  // ── M5: Import handlers ──

  const handleImportTasks = (tasks: ExternalTask[]) => {
    setImportTasks(tasks);
    setSelectedImportIds(new Set(tasks.map((t) => t.external_id)));
    setShowIntegrations(false);
    setShowImportPreview(true);
  };

  const toggleImportSelection = (id: string) => {
    setSelectedImportIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleConfirmImport = async () => {
    const toCreate = importTasks.filter((t) => selectedImportIds.has(t.external_id));
    for (const task of toCreate) {
      const priority = (['A', 'B', 'C'].includes(task.priority_hint)
        ? task.priority_hint
        : 'B') as Task['priority'];
      await createTask(task.title, null, priority, 60);
    }
    setShowImportPreview(false);
    setImportTasks([]);
    setSelectedImportIds(new Set());
    refreshTasks();
  };

  // ── M6: Auto reply handlers ──

  const loadPendingReplies = useCallback(async () => {
    try {
      const replies = await getPendingReplies();
      setPendingReplies(replies);
    } catch {
      // silently ignore
    }
  }, []);

  useEffect(() => {
    loadPendingReplies();
  }, [loadPendingReplies]);

  // Simulate incoming message during focus (for demo)
  const simulateIncomingMessage = async () => {
    if (!activeFocus) return;
    try {
      await generateAutoReply('在吗？下午的会议方案改到3点了，你准备一下。', '微信');
      await generateAutoReply('Hey, can you review my PR when you have a moment?', 'Slack');
      await loadPendingReplies();
    } catch {
      // silently ignore
    }
  };

  // ── Display order ──

  const displayTasks = orderedTaskIds
    ? [...tasks].sort((a, b) => {
        const ai = orderedTaskIds.indexOf(a.id);
        const bi = orderedTaskIds.indexOf(b.id);
        if (ai === -1 && bi === -1) return 0;
        if (ai === -1) return 1;
        if (bi === -1) return -1;
        return ai - bi;
      })
    : tasks;

  return (
    <div className="flex h-screen bg-gray-50 text-gray-900 dark:bg-gray-950 dark:text-gray-100">
      <Sidebar
        projects={projects}
        activeProjectId={activeProjectId}
        onSelect={setActiveProjectId}
        onProjectCreated={handleProjectCreated}
        onProjectRenamed={handleProjectRenamed}
        onProjectDeleted={handleProjectDeleted}
        showNewProject={showNewProject}
        setShowNewProject={setShowNewProject}
        onIntegrations={() => setShowIntegrations(true)}
        onAutoReply={() => setShowAutoReply(true)}
        onReview={() => setShowReview(true)}
        onSettings={() => setShowSettings(true)}
        pendingReplyCount={pendingReplies.length}
      />
      <main className="flex-1 flex flex-col overflow-hidden">
        <Timeline
          tasks={displayTasks}
          projects={projects}
          activeProjectId={activeProjectId}
          onEditTask={setEditingTask}
          onTaskCreated={refreshTasks}
          onAiReorder={handleAiReorder}
          isReordering={isReordering}
          isAiOrdered={orderedTaskIds !== null}
          onNlSubmit={handleNlSubmit}
          isParsing={isParsing}
          parseError={parseError}
          activeFocus={activeFocus}
          onStartFocus={handleStartFocus}
          calibrationRatio={calibrationRatio}
          peakHours={peakHours}
        />
      </main>

      {/* M2: NL parsing → create modal */}
      {parsingTask && (
        <TaskModal
          mode="create"
          prefill={{
            title: parsingTask.title,
            priority: parsingTask.priority as Task['priority'],
            estimatedDurationMin: parsingTask.duration_min,
          }}
          projectHint={parsingTask.project_hint}
          projects={projects}
          onClose={() => {
            setParsingTask(null);
            setParseError(null);
          }}
          onCreate={handleNlCreate}
        />
      )}

      {/* Existing task edit modal */}
      {editingTask && !parsingTask && (
        <TaskModal
          mode="edit"
          task={editingTask}
          projects={projects}
          onClose={() => setEditingTask(null)}
          onUpdated={refreshTasks}
          onDeleted={refreshTasks}
        />
      )}

      {/* M3: Focus mode overlay */}
      {activeFocus && (
        <FocusMode
          sessionId={activeFocus.id}
          taskId={activeFocus.task_id}
          taskTitle={activeFocus.task_title}
          startTime={activeFocus.start_time}
          status={activeFocus.status}
          onEnd={handleEndFocus}
          onPause={handlePauseFocus}
          onResume={handleResumeFocus}
        />
      )}

      {/* P0-1: Peak-hours linkage note (auto-dismiss) */}
      {peakNote && (
        <div className="fixed top-4 left-1/2 -translate-x-1/2 z-[60] px-4 py-2 rounded-full bg-amber-100 text-amber-800 text-sm shadow-md border border-amber-200 dark:bg-amber-900/60 dark:text-amber-100 dark:border-amber-700">
          {peakNote}
        </div>
      )}

      {/* M3: Focus summary dialog */}
      {focusSummary && (
        <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50">
          <div className="bg-white rounded-xl shadow-xl w-full max-w-sm p-6 mx-4">
            <h3 className="text-lg font-semibold mb-2">专注结束</h3>
            <p className="text-gray-600 text-sm mb-1">
              专注了 {Math.round(focusSummary.duration_seconds / 60)} 分钟
            </p>
            {focusSummary.task_id && (
              <p className="text-gray-500 text-xs mb-4">
                是否将关联任务标记为完成？
              </p>
            )}
            {pendingReplies.length > 0 && (
              <p className="text-blue-600 text-xs mb-3 bg-blue-50 p-2 rounded">
                AI 为你起草了 {pendingReplies.length} 条回复，可点击左下角"AI 回复"查看
              </p>
            )}
            <div className="flex gap-2 justify-end">
              <button
                className="px-4 py-2 text-sm text-gray-500 hover:bg-gray-100 rounded-lg transition-colors"
                onClick={() => handleFocusSummaryClose(false)}
              >
                不用了
              </button>
              {focusSummary.task_id && (
                <button
                  className="px-4 py-2 text-sm bg-green-500 text-white rounded-lg hover:bg-green-600 transition-colors"
                  onClick={() => handleFocusSummaryClose(true)}
                >
                  标记完成
                </button>
              )}
            </div>
          </div>
        </div>
      )}

      {/* M5: Integrations panel */}
      <IntegrationsPanel
        isOpen={showIntegrations}
        onClose={() => setShowIntegrations(false)}
        onImport={handleImportTasks}
      />

      {/* M5: Import preview modal */}
      {showImportPreview && importTasks.length > 0 && (
        <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50">
          <div className="bg-white rounded-xl shadow-xl w-full max-w-md p-6 mx-4 max-h-[80vh] flex flex-col">
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-lg font-semibold">选择要导入的任务</h3>
              <button
                className="text-gray-400 hover:text-gray-600"
                onClick={() => {
                  setShowImportPreview(false);
                  setImportTasks([]);
                }}
              >
                <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
            <p className="text-xs text-gray-500 mb-3">
              共 {importTasks.length} 条外部任务，已全选。取消勾选不需要导入的。
            </p>
            <div className="flex-1 overflow-auto space-y-1 mb-4">
              {importTasks.map((task) => (
                <label
                  key={task.external_id}
                  className="flex items-center gap-3 p-2 rounded-lg hover:bg-gray-50 cursor-pointer"
                >
                  <input
                    type="checkbox"
                    className="w-4 h-4 rounded border-gray-300 text-blue-600"
                    checked={selectedImportIds.has(task.external_id)}
                    onChange={() => toggleImportSelection(task.external_id)}
                  />
                  <div className="flex-1 min-w-0">
                    <p className="text-sm text-gray-800 truncate">{task.title}</p>
                    <div className="flex items-center gap-2 mt-0.5">
                      <span className="text-[10px] text-gray-400 uppercase">{task.source}</span>
                      <span className="text-[10px] text-gray-400">优先级 {task.priority_hint}</span>
                    </div>
                  </div>
                </label>
              ))}
            </div>
            <div className="flex gap-2 justify-end">
              <button
                className="px-4 py-2 text-sm text-gray-500 hover:bg-gray-100 rounded-lg"
                onClick={() => {
                  setShowImportPreview(false);
                  setImportTasks([]);
                }}
              >
                取消
              </button>
              <button
                className="px-4 py-2 text-sm bg-blue-500 text-white rounded-lg hover:bg-blue-600"
                onClick={handleConfirmImport}
              >
                导入 {selectedImportIds.size} 条
              </button>
            </div>
          </div>
        </div>
      )}

      {/* M6: Auto reply panel */}
      {showAutoReply && (
        <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50">
          <div className="bg-white rounded-xl shadow-xl w-full max-w-md p-6 mx-4 max-h-[80vh] flex flex-col">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold">AI 回复草稿</h3>
              <button
                className="text-gray-400 hover:text-gray-600"
                onClick={() => setShowAutoReply(false)}
              >
                <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
            <div className="flex-1 overflow-auto">
              <AutoReplyPanel replies={pendingReplies} onRefresh={loadPendingReplies} />
            </div>
            <button
              className="mt-3 w-full px-3 py-1.5 text-xs text-blue-500 hover:bg-blue-50 rounded-lg border border-blue-200 transition-colors"
              onClick={simulateIncomingMessage}
              disabled={!activeFocus}
              title={activeFocus ? '模拟收到新消息' : '需在专注模式下使用'}
            >
              {activeFocus ? '模拟收到新消息（演示）' : '需在专注模式中使用'}
            </button>
          </div>
        </div>
      )}

      {/* M7: Weekly report dashboard */}
      {showReview && <WeeklyReport onClose={() => setShowReview(false)} />}

      {/* M9: Settings (theme + app categories) */}
      {showSettings && (
        <SettingsModal
          isOpen={showSettings}
          theme={theme}
          onToggleTheme={() => setTheme((t) => (t === 'dark' ? 'light' : 'dark'))}
          onClose={() => setShowSettings(false)}
        />
      )}
    </div>
  );
}