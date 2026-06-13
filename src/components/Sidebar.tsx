import { useState, useRef, useEffect } from 'react';
import type { Project } from '../types';

interface SidebarProps {
  projects: Project[];
  activeProjectId: string | null;
  onSelect: (id: string | null) => void;
  onProjectCreated: (name: string) => void;
  onProjectRenamed: (id: string, name: string) => void;
  onProjectDeleted: (id: string) => void;
  showNewProject: boolean;
  setShowNewProject: (v: boolean) => void;
  onIntegrations: () => void;
  onAutoReply: () => void;
  onReview: () => void;
  pendingReplyCount: number;
}

export default function Sidebar({
  projects,
  activeProjectId,
  onSelect,
  onProjectCreated,
  onProjectRenamed,
  onProjectDeleted,
  showNewProject,
  setShowNewProject,
  onIntegrations,
  onAutoReply,
  onReview,
  pendingReplyCount,
}: SidebarProps) {
  const [newName, setNewName] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);
  const editRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (showNewProject) inputRef.current?.focus();
  }, [showNewProject]);

  useEffect(() => {
    if (editingId) editRef.current?.focus();
  }, [editingId]);

  const handleCreate = () => {
    const name = newName.trim();
    if (!name) return;
    onProjectCreated(name);
    setNewName('');
  };

  const handleRename = (id: string) => {
    const name = editName.trim();
    if (!name || name === projects.find((p) => p.id === id)?.name) {
      setEditingId(null);
      return;
    }
    onProjectRenamed(id, name);
    setEditingId(null);
  };

  return (
    <aside className="w-56 bg-white border-r border-gray-200 flex flex-col shrink-0">
      <div className="px-4 py-3 border-b border-gray-100">
        <h1 className="text-lg font-semibold tracking-tight">flowtime</h1>
        <p className="text-xs text-gray-400 mt-0.5">AI 时间管理</p>
      </div>

      <div className="flex-1 overflow-y-auto px-2 py-2">
        <button
          className={`w-full text-left px-3 py-1.5 rounded-md text-sm mb-1 transition-colors
            ${activeProjectId === null ? 'bg-blue-50 text-blue-700 font-medium' : 'text-gray-600 hover:bg-gray-50'}`}
          onClick={() => onSelect(null)}
        >
          所有任务
        </button>

        <div className="mt-3 mb-1 px-2 text-xs font-medium text-gray-400 uppercase tracking-wider">
          项目
        </div>

        {projects.map((p) =>
          editingId === p.id ? (
            <div key={p.id} className="px-1 py-1">
              <input
                ref={editRef}
                className="w-full px-2 py-1 text-sm border border-blue-300 rounded-md outline-none focus:ring-1 focus:ring-blue-400"
                value={editName}
                onChange={(e) => setEditName(e.target.value)}
                onBlur={() => handleRename(p.id)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') handleRename(p.id);
                  if (e.key === 'Escape') setEditingId(null);
                }}
              />
            </div>
          ) : (
            <div
              key={p.id}
              className={`group flex items-center gap-2 px-3 py-1.5 rounded-md text-sm cursor-pointer transition-colors
                ${activeProjectId === p.id ? 'bg-blue-50 text-blue-700 font-medium' : 'text-gray-700 hover:bg-gray-50'}`}
              onClick={() => onSelect(p.id)}
            >
              <span
                className="w-2.5 h-2.5 rounded-full shrink-0"
                style={{ backgroundColor: p.color }}
              />
              <span className="flex-1 truncate">{p.name}</span>
              <div className="hidden group-hover:flex items-center gap-0.5">
                <button
                  className="text-gray-400 hover:text-gray-600 p-0.5"
                  onClick={(e) => {
                    e.stopPropagation();
                    setEditName(p.name);
                    setEditingId(p.id);
                  }}
                  title="重命名"
                >
                  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z" />
                  </svg>
                </button>
                <button
                  className="text-gray-400 hover:text-red-500 p-0.5"
                  onClick={(e) => {
                    e.stopPropagation();
                    if (window.confirm(`确定删除项目「${p.name}」？`)) {
                      onProjectDeleted(p.id);
                    }
                  }}
                  title="删除项目"
                >
                  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                  </svg>
                </button>
              </div>
            </div>
          ),
        )}

        {showNewProject ? (
          <div className="px-1 py-1 mt-1">
            <input
              ref={inputRef}
              className="w-full px-2 py-1 text-sm border border-blue-300 rounded-md outline-none focus:ring-1 focus:ring-blue-400"
              placeholder="项目名称，回车创建"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onBlur={() => {
                if (!newName.trim()) setShowNewProject(false);
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleCreate();
                if (e.key === 'Escape') {
                  setShowNewProject(false);
                  setNewName('');
                }
              }}
            />
          </div>
        ) : (
          <button
            className="w-full text-left px-3 py-1.5 rounded-md text-sm text-gray-400 hover:text-gray-600 hover:bg-gray-50 mt-1 transition-colors"
            onClick={() => setShowNewProject(true)}
          >
            + 新建项目
          </button>
        )}
      </div>

      <div className="px-3 py-2 border-t border-gray-200 space-y-1">
        <button
          className="w-full text-left px-3 py-2 rounded-lg text-sm text-gray-600 hover:bg-gray-100 transition-colors flex items-center gap-2"
          onClick={onReview}
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
          </svg>
          复盘看板
        </button>
        <button
          className="w-full text-left px-3 py-2 rounded-lg text-sm text-gray-600 hover:bg-gray-100 transition-colors flex items-center gap-2"
          onClick={onIntegrations}
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M11 4a2 2 0 114 0v1a1 1 0 001 1h3a1 1 0 011 1v3a1 1 0 01-1 1h-1a2 2 0 100 4h1a1 1 0 011 1v3a1 1 0 01-1 1h-3a1 1 0 01-1-1v-1a2 2 0 10-4 0v1a1 1 0 01-1 1H7a1 1 0 01-1-1v-3a1 1 0 00-1-1H4a2 2 0 110-4h1a1 1 0 001-1V7a1 1 0 011-1h3a1 1 0 001-1V4z" />
          </svg>
          外部集成
        </button>
        <button
          className="w-full text-left px-3 py-2 rounded-lg text-sm text-gray-600 hover:bg-gray-100 transition-colors flex items-center gap-2"
          onClick={onAutoReply}
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6" />
          </svg>
          AI 回复
          {pendingReplyCount > 0 && (
            <span className="ml-auto bg-blue-500 text-white text-[10px] px-1.5 py-0.5 rounded-full">
              {pendingReplyCount}
            </span>
          )}
        </button>
      </div>
    </aside>
  );
}
