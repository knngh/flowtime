import { useState } from 'react';
import type { ExternalTask, IntegrationSource, IntegrationStatus } from '../types';
import { importExternalTasks } from '../integrations';

interface IntegrationsPanelProps {
  isOpen: boolean;
  onClose: () => void;
  onImport: (tasks: ExternalTask[]) => void;
}

const INTEGRATIONS: Omit<IntegrationStatus, 'loading' | 'connected'>[] = [
  { source: 'github', label: 'GitHub Issues', env_var: 'GITHUB_TOKEN' },
  { source: 'linear', label: 'Linear', env_var: 'LINEAR_API_KEY' },
  { source: 'feishu', label: '飞书日历', env_var: 'FEISHU_APP_ID' },
];

export default function IntegrationsPanel({ isOpen, onClose, onImport }: IntegrationsPanelProps) {
  const [statuses, setStatuses] = useState<IntegrationStatus[]>(
    INTEGRATIONS.map((i) => ({ ...i, connected: false, loading: false })),
  );
  const [selectedSources, setSelectedSources] = useState<Set<IntegrationSource>>(new Set());
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!isOpen) return null;

  const toggleSource = (source: IntegrationSource) => {
    setSelectedSources((prev) => {
      const next = new Set(prev);
      if (next.has(source)) next.delete(source);
      else next.add(source);
      return next;
    });
  };

  const handleImport = async () => {
    if (selectedSources.size === 0) return;
    setIsLoading(true);
    setError(null);
    try {
      const sources = Array.from(selectedSources);
      const result = await importExternalTasks(sources);
      // Mark sources as connected (based on whether they had tasks or non-auth errors)
      setStatuses((prev) =>
        prev.map((s) => {
          const src = s.source as IntegrationSource;
          const hasTasks = result.tasks.some((t) => t.source === src);
          return { ...s, connected: hasTasks ? true : s.connected };
        }),
      );
      // Show partial failures as info, not blocking error
      if (result.errors.length > 0) {
        const errorMsg = result.errors
          .map((e) => `${e.source}: ${e.message}`)
          .join('; ');
        setError(errorMsg);
      }
      onImport(result.tasks);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-40">
      <div className="bg-white rounded-xl shadow-xl w-full max-w-md p-6 mx-4">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold">外部集成</h3>
          <button
            className="text-gray-400 hover:text-gray-600 transition-colors"
            onClick={onClose}
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <p className="text-sm text-gray-500 mb-4">
          选择外部数据源，拉取你的任务和事件。API 密钥通过环境变量配置，仅读取不写入。
        </p>

        <div className="space-y-2 mb-4">
          {statuses.map((s) => (
            <label
              key={s.source}
              className={`flex items-center gap-3 p-3 rounded-lg border cursor-pointer transition-colors ${
                selectedSources.has(s.source as IntegrationSource)
                  ? 'border-blue-300 bg-blue-50'
                  : 'border-gray-200 hover:border-gray-300'
              }`}
            >
              <input
                type="checkbox"
                className="w-4 h-4 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                checked={selectedSources.has(s.source as IntegrationSource)}
                onChange={() => toggleSource(s.source as IntegrationSource)}
              />
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium text-gray-800">{s.label}</span>
                  {s.connected && (
                    <span className="text-[10px] text-green-600 bg-green-50 px-1.5 py-0.5 rounded-full">
                      已连接
                    </span>
                  )}
                </div>
                <p className="text-xs text-gray-400">环境变量: {s.env_var}</p>
              </div>
            </label>
          ))}
        </div>

        {error && (
          <p className="text-xs text-red-500 mb-3 bg-red-50 p-2 rounded">{error}</p>
        )}

        <div className="flex gap-2 justify-end">
          <button
            className="px-4 py-2 text-sm text-gray-500 hover:bg-gray-100 rounded-lg transition-colors"
            onClick={onClose}
          >
            取消
          </button>
          <button
            className={`px-4 py-2 text-sm rounded-lg transition-colors ${
              selectedSources.size === 0 || isLoading
                ? 'bg-gray-200 text-gray-400 cursor-not-allowed'
                : 'bg-blue-500 text-white hover:bg-blue-600'
            }`}
            disabled={selectedSources.size === 0 || isLoading}
            onClick={handleImport}
          >
            {isLoading ? (
              <span className="flex items-center gap-1.5">
                <div className="w-3 h-3 border-2 border-white border-t-transparent rounded-full animate-spin" />
                拉取中…
              </span>
            ) : (
              '拉取任务'
            )}
          </button>
        </div>
      </div>
    </div>
  );
}