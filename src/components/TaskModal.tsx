import { useState, useEffect, useRef } from 'react';
import type { Project, Task } from '../types';
import { STATUS_LABELS, PRIORITY_LABELS } from '../types';
import { updateTask, deleteTask } from '../db';

interface PrefillData {
  title: string;
  priority: Task['priority'];
  estimatedDurationMin: number;
}

interface CreateResult {
  title: string;
  priority: string;
  estimatedDurationMin: number;
  projectId: string | null;
}

interface TaskModalBase {
  projects: Project[];
  onClose: () => void;
}

interface EditMode extends TaskModalBase {
  mode: 'edit';
  task: Task;
  onUpdated: () => void;
  onDeleted: () => void;
  onCreate?: never;
  prefill?: never;
  projectHint?: never;
}

interface CreateMode extends TaskModalBase {
  mode: 'create';
  task?: never;
  onUpdated?: never;
  onDeleted?: never;
  onCreate: (result: CreateResult) => void;
  prefill: PrefillData;
  projectHint?: string | null;
}

type TaskModalProps = EditMode | CreateMode;

/** Fuzzy match project_hint against project names */
function matchProject(hint: string | null | undefined, projects: Project[]): string | null {
  if (!hint) return null;
  const lower = hint.toLowerCase().trim();
  if (!lower) return null;
  // Exact match first
  const exact = projects.find((p) => p.name.toLowerCase() === lower);
  if (exact) return exact.id;
  // Contains match
  const contains = projects.find((p) => p.name.toLowerCase().includes(lower));
  if (contains) return contains.id;
  // Reverse contains
  const reverse = projects.find((p) => lower.includes(p.name.toLowerCase()));
  if (reverse) return reverse.id;
  return null;
}

export default function TaskModal(props: TaskModalProps) {
  const { projects, onClose } = props;

  // Initial values depend on mode
  const initialTitle = props.mode === 'edit' ? props.task.title : props.prefill.title;
  const initialPriority = (props.mode === 'edit' ? props.task.priority : props.prefill.priority) as Task['priority'];
  const initialDuration =
    props.mode === 'edit' ? props.task.estimated_duration_min : props.prefill.estimatedDurationMin;
  const initialStatus = props.mode === 'edit' ? props.task.status : ('todo' as Task['status']);
  const initialProjectId = props.mode === 'edit' ? props.task.project_id : matchProject(props.projectHint, projects);

  const [title, setTitle] = useState(initialTitle);
  const [priority, setPriority] = useState<Task['priority']>(initialPriority);
  const [status, setStatus] = useState<Task['status']>(initialStatus);
  const [duration, setDuration] = useState(initialDuration);
  const [projectId, setProjectId] = useState<string | null>(initialProjectId);

  const overlayRef = useRef<HTMLDivElement>(null);
  const isCreate = props.mode === 'create';

  useEffect(() => {
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (isCreate) {
          onClose();
        } else {
          saveAndClose();
        }
      }
    };
    window.addEventListener('keydown', handleEsc);
    return () => window.removeEventListener('keydown', handleEsc);
  });

  const saveAndClose = async () => {
    if (props.mode !== 'edit') return;
    const trimmed = title.trim();
    if (!trimmed) return;
    const changed: Partial<
      Pick<Task, 'title' | 'priority' | 'estimated_duration_min' | 'project_id' | 'status'>
    > = {};
    if (trimmed !== props.task.title) changed.title = trimmed;
    if (priority !== props.task.priority) changed.priority = priority;
    if (duration !== props.task.estimated_duration_min) changed.estimated_duration_min = duration;
    if (projectId !== props.task.project_id) changed.project_id = projectId ?? undefined;
    if (status !== props.task.status) changed.status = status;

    if (Object.keys(changed).length > 0) {
      await updateTask(props.task.id, changed);
    }
    props.onUpdated();
  };

  const handleCreate = () => {
    if (props.mode !== 'create') return;
    const trimmed = title.trim();
    if (!trimmed) return;
    props.onCreate({
      title: trimmed,
      priority,
      estimatedDurationMin: duration,
      projectId,
    });
  };

  const handleDelete = async () => {
    if (props.mode !== 'edit') return;
    if (window.confirm('确定删除此任务？此操作不可撤销。')) {
      await deleteTask(props.task.id);
      props.onDeleted();
    }
  };

  const titleLabel = isCreate ? '新建任务' : '编辑任务';
  const confirmLabel = isCreate ? '创建' : '保存';
  const confirmAction = isCreate ? handleCreate : saveAndClose;

  const handleEnter = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') confirmAction();
  };

  return (
    <div
      ref={overlayRef}
      className="fixed inset-0 bg-black/30 flex items-center justify-center z-50"
      onClick={(e) => {
        if (e.target === overlayRef.current) {
          if (isCreate) onClose();
          else saveAndClose();
        }
      }}
    >
      <div className="bg-white rounded-xl shadow-xl w-full max-w-md p-6 mx-4">
        <h3 className="text-base font-semibold mb-4">{titleLabel}</h3>

        {/* Title */}
        <label className="block text-xs font-medium text-gray-500 mb-1">任务名称</label>
        <input
          className="w-full px-3 py-2 text-sm border border-gray-200 rounded-lg outline-none focus:ring-2 focus:ring-purple-200 focus:border-purple-400 mb-3"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={handleEnter}
          autoFocus
        />

        {/* Priority */}
        <label className="block text-xs font-medium text-gray-500 mb-1">优先级</label>
        <div className="flex gap-2 mb-3">
          {(['A', 'B', 'C'] as const).map((p) => (
            <button
              key={p}
              className={`px-3 py-1.5 text-sm rounded-md border transition-colors
                ${priority === p ? 'border-purple-400 bg-purple-50 text-purple-700 font-medium' : 'border-gray-200 text-gray-600 hover:bg-gray-50'}`}
              onClick={() => setPriority(p)}
            >
              {p} - {PRIORITY_LABELS[p]}
            </button>
          ))}
        </div>

        {/* Status (edit only) */}
        {!isCreate && (
          <>
            <label className="block text-xs font-medium text-gray-500 mb-1">状态</label>
            <div className="flex flex-wrap gap-2 mb-3">
              {(['todo', 'in_progress', 'done', 'deferred'] as const).map((s) => (
                <button
                  key={s}
                  className={`px-3 py-1.5 text-sm rounded-md border transition-colors
                    ${status === s ? 'border-blue-400 bg-blue-50 text-blue-700 font-medium' : 'border-gray-200 text-gray-600 hover:bg-gray-50'}`}
                  onClick={() => setStatus(s)}
                >
                  {STATUS_LABELS[s]}
                </button>
              ))}
            </div>
          </>
        )}

        {/* Duration */}
        <label className="block text-xs font-medium text-gray-500 mb-1">预计时长（分钟）</label>
        <select
          className="w-full px-3 py-2 text-sm border border-gray-200 rounded-lg outline-none focus:ring-2 focus:ring-purple-200 mb-3"
          value={duration}
          onChange={(e) => setDuration(Number(e.target.value))}
        >
          <option value={15}>15 分钟</option>
          <option value={30}>30 分钟</option>
          <option value={45}>45 分钟</option>
          <option value={60}>1 小时</option>
          <option value={90}>1.5 小时</option>
          <option value={120}>2 小时</option>
          <option value={180}>3 小时</option>
          <option value={240}>4 小时</option>
        </select>

        {/* Project */}
        <label className="block text-xs font-medium text-gray-500 mb-1">所属项目</label>
        <select
          className="w-full px-3 py-2 text-sm border border-gray-200 rounded-lg outline-none focus:ring-2 focus:ring-purple-200 mb-4"
          value={projectId ?? ''}
          onChange={(e) => setProjectId(e.target.value || null)}
        >
          <option value="">无项目</option>
          {projects.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>

        {/* Actions */}
        <div className="flex items-center justify-between">
          {isCreate ? (
            <div />
          ) : (
            <button
              className="px-3 py-1.5 text-sm text-red-500 hover:bg-red-50 rounded-md transition-colors"
              onClick={handleDelete}
            >
              删除任务
            </button>
          )}
          <div className="flex gap-2">
            <button
              className="px-4 py-1.5 text-sm text-gray-500 hover:bg-gray-50 rounded-md border border-gray-200 transition-colors"
              onClick={onClose}
            >
              取消
            </button>
            <button
              className="px-4 py-1.5 text-sm bg-purple-500 text-white rounded-md hover:bg-purple-600 transition-colors disabled:opacity-40"
              disabled={!title.trim()}
              onClick={confirmAction}
            >
              {confirmLabel}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}