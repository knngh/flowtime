import type { Project, Task, ActiveFocusSession, PeakHoursSuggestion } from '../types';
import { STATUS_LABELS, PRIORITY_LABELS } from '../types';
import { deleteTask, updateTask } from '../db';
import InputBar from './InputBar';

interface TimelineProps {
  tasks: Task[];
  projects: Project[];
  activeProjectId: string | null;
  onEditTask: (t: Task) => void;
  onTaskCreated: () => void;
  // M2: AI reorder
  onAiReorder: () => void;
  isReordering: boolean;
  isAiOrdered: boolean;
  // M2: NL parsing
  onNlSubmit: (input: string) => void;
  isParsing: boolean;
  parseError: string | null;
  // M3: Focus mode
  activeFocus: ActiveFocusSession | null;
  onStartFocus: (taskId: string, taskTitle: string) => void;
  // M8: Behavior learning
  calibrationRatio: number;
  peakHours: PeakHoursSuggestion | null;
}

function getTodayDate(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

function getStatusColor(status: Task['status']): string {
  switch (status) {
    case 'done':
      return 'text-green-600 bg-green-50';
    case 'in_progress':
      return 'text-blue-600 bg-blue-50';
    case 'deferred':
      return 'text-amber-600 bg-amber-50';
    default:
      return 'text-gray-500 bg-gray-100';
  }
}

export default function Timeline({
  tasks,
  projects,
  activeProjectId,
  onEditTask,
  onTaskCreated,
  onAiReorder,
  isReordering,
  isAiOrdered,
  onNlSubmit,
  isParsing,
  parseError,
  activeFocus,
  onStartFocus,
  calibrationRatio,
  peakHours,
}: TimelineProps) {
  const today = getTodayDate();
  const weekDay = ['日', '一', '二', '三', '四', '五', '六'][new Date().getDay()];
  const getProject = (pid: string | null) => projects.find((p) => p.id === pid);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="px-6 py-4 border-b border-gray-100 bg-white">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <h2 className="text-lg font-semibold">
              今天 · {today.split('-')[1]}月{today.split('-')[2]}日 · 周{weekDay}
            </h2>
            <span className="text-xs text-purple-500 bg-purple-50 px-2 py-0.5 rounded-full">M2</span>
            {activeFocus && (
              <span className="text-xs text-red-500 bg-red-50 px-2 py-0.5 rounded-full animate-pulse">
                专注中
              </span>
            )}
          </div>
          {tasks.length > 0 && (
            <button
              className={`flex items-center gap-1.5 text-xs px-3 py-1.5 rounded-md transition-colors ${
                isAiOrdered
                  ? 'bg-purple-100 text-purple-700'
                  : 'bg-white border border-gray-200 text-gray-600 hover:border-purple-300 hover:text-purple-600'
              } disabled:opacity-50`}
              onClick={onAiReorder}
              disabled={isReordering}
              title="AI 智能排程"
            >
              {isReordering ? (
                <>
                  <div className="w-3 h-3 border-2 border-purple-300 border-t-purple-600 rounded-full animate-spin" />
                  <span>分析中…</span>
                </>
              ) : (
                <>
                  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth="2"
                      d="M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09zM18.259 8.715L18 9.75l-.259-1.035a3.375 3.375 0 00-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 002.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 002.455 2.456L21.75 6l-1.036.259a3.375 3.375 0 00-2.455 2.456zM16.894 20.567L16.5 21.75l-.394-1.183a2.25 2.25 0 00-1.423-1.423L13.5 18.75l1.183-.394a2.25 2.25 0 001.423-1.423l.394-1.183.394 1.183a2.25 2.25 0 001.423 1.423l1.183.394-1.183.394a2.25 2.25 0 00-1.423 1.423z"
                    />
                  </svg>
                  <span>AI 重排</span>
                </>
              )}
            </button>
          )}
        </div>
      </div>

      {/* Timeline */}
      <div className="flex-1 overflow-y-auto px-6 py-4">
        {tasks.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-gray-400">
            <svg className="w-12 h-12 mb-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth="1.5"
                d="M12 6v6l4 2m6-2a10 10 0 11-20 0 10 10 0 0120 0z"
              />
            </svg>
            <p className="text-sm">暂无任务，在底部输入框使用自然语言创建</p>
          </div>
        ) : (
          <div className="space-y-2">
            {tasks.map((task, idx) => {
              const project = getProject(task.project_id);
              const isFocusingThis = activeFocus?.task_id === task.id;
              return (
                <div
                  key={task.id}
                  className={`group flex items-start gap-3 p-3 rounded-lg border transition-all ${
                    isFocusingThis
                      ? 'bg-red-50 border-red-200 shadow-sm'
                      : 'bg-white border-gray-100 hover:border-gray-200 hover:shadow-sm cursor-pointer'
                  }`}
                  onClick={() => !isFocusingThis && onEditTask(task)}
                >
                  {/* Order indicator when AI ordered */}
                  {isAiOrdered && (
                    <span className="text-[10px] text-purple-400 shrink-0 w-5 text-center leading-6">
                      {idx + 1}
                    </span>
                  )}

                  {/* Status checkbox */}
                  <button
                    className="mt-0.5 shrink-0"
                    onClick={(e) => {
                      e.stopPropagation();
                      const newStatus = task.status === 'done' ? 'todo' : 'done';
                      updateTask(task.id, { status: newStatus }).then(onTaskCreated);
                    }}
                    title={task.status === 'done' ? '标记为待办' : '标记为完成'}
                  >
                    <div
                      className={`w-5 h-5 rounded-full border-2 flex items-center justify-center transition-colors
                        ${task.status === 'done' ? 'bg-green-500 border-green-500' : 'border-gray-300 hover:border-blue-400'}`}
                    >
                      {task.status === 'done' && (
                        <svg
                          className="w-3 h-3 text-white"
                          fill="none"
                          stroke="currentColor"
                          viewBox="0 0 24 24"
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            strokeWidth="3"
                            d="M5 13l4 4L19 7"
                          />
                        </svg>
                      )}
                    </div>
                  </button>

                  {/* Content */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span
                        className={`text-sm font-medium ${task.status === 'done' ? 'line-through text-gray-400' : 'text-gray-800'}`}
                      >
                        {task.title}
                      </span>
                      <span
                        className={`text-[10px] px-1.5 py-0.5 rounded ${getStatusColor(task.status)}`}
                      >
                        {STATUS_LABELS[task.status]}
                      </span>
                      {isFocusingThis && (
                        <span className="text-[10px] text-red-500 bg-red-100 px-1.5 py-0.5 rounded animate-pulse">
                          专注中
                        </span>
                      )}
                    </div>
                    <div className="flex items-center gap-2 mt-1">
                      {project && (
                        <span className="flex items-center gap-1 text-xs text-gray-400">
                          <span className="w-2 h-2 rounded-full" style={{ backgroundColor: project.color }} />
                          {project.name}
                        </span>
                      )}
                      <span className="text-xs text-gray-400">
                        {PRIORITY_LABELS[task.priority]}优先级 · {task.estimated_duration_min}分钟
                      </span>
                      {task.scheduled_start && (
                        <span className="text-xs text-gray-400">
                          {task.scheduled_start.slice(11, 16)} —{' '}
                          {task.scheduled_end?.slice(11, 16) || '...'}
                        </span>
                      )}
                    </div>
                  </div>

                  {/* Start focus button */}
                  {!isFocusingThis && task.status !== 'done' && (
                    <button
                      className="hidden group-hover:flex items-center justify-center w-7 h-7 rounded-full bg-red-50 hover:bg-red-100 text-red-500 shrink-0 transition-colors"
                      onClick={(e) => {
                        e.stopPropagation();
                        onStartFocus(task.id, task.title);
                      }}
                      title="开始专注"
                      disabled={activeFocus !== null}
                    >
                      <svg className="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24">
                        <path d="M8 5.14v14l11-7-11-7z" />
                      </svg>
                    </button>
                  )}

                  {/* Quick delete */}
                  <button
                    className="hidden group-hover:block text-gray-300 hover:text-red-500 shrink-0 p-0.5"
                    onClick={(e) => {
                      e.stopPropagation();
                      if (window.confirm('确定删除此任务？')) {
                        deleteTask(task.id).then(onTaskCreated);
                      }
                    }}
                    title="删除任务"
                  >
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth="2"
                        d="M6 18L18 6M6 6l12 12"
                      />
                    </svg>
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* M8: Peak hours insight */}
      {peakHours && peakHours.peak_hours.length > 0 && (
        <div className="px-6 py-2 bg-amber-50 border-t border-amber-100">
          <p className="text-xs text-amber-700">
            {peakHours.insight}
          </p>
        </div>
      )}

      {/* Input bar */}
      <InputBar
        activeProjectId={activeProjectId}
        onCreated={onTaskCreated}
        onNlSubmit={onNlSubmit}
        isParsing={isParsing}
        parseError={parseError}
        calibrationRatio={calibrationRatio}
      />
    </div>
  );
}
