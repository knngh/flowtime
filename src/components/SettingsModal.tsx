import { useState, useEffect, useCallback } from 'react';
import { getAppCategories, setAppCategory, deleteAppCategory } from '../review';

interface SettingsModalProps {
  isOpen: boolean;
  theme: 'light' | 'dark';
  onToggleTheme: () => void;
  onClose: () => void;
}

const PRESET_CATEGORIES = [
  'coding',
  'meeting',
  'communication',
  'design',
  'browsing',
  'entertainment',
  'other',
];

export default function SettingsModal({ isOpen, theme, onToggleTheme, onClose }: SettingsModalProps) {
  const [categories, setCategories] = useState<Record<string, string>>({});
  const [appInput, setAppInput] = useState('');
  const [categoryInput, setCategoryInput] = useState('coding');
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const cats = await getAppCategories();
      setCategories(cats);
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    if (isOpen) refresh();
  }, [isOpen, refresh]);

  const handleAdd = async () => {
    const app = appInput.trim();
    if (!app) return;
    setLoading(true);
    try {
      await setAppCategory(app, categoryInput);
      setAppInput('');
      await refresh();
    } catch (e) {
      console.error('Failed to set category:', e);
    } finally {
      setLoading(false);
    }
  };

  const handleDelete = async (app: string) => {
    try {
      await deleteAppCategory(app);
      await refresh();
    } catch (e) {
      console.error('Failed to delete category:', e);
    }
  };

  if (!isOpen) return null;

  const entries = Object.entries(categories);

  return (
    <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-[70]">
      <div className="bg-white rounded-xl shadow-xl w-full max-w-md p-6 mx-4 max-h-[85vh] flex flex-col dark:bg-gray-800 dark:text-gray-100">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold">设置</h3>
          <button
            className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
            onClick={onClose}
            aria-label="关闭"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Theme toggle */}
        <div className="flex items-center justify-between py-3 border-b border-gray-100 dark:border-gray-700">
          <div>
            <p className="text-sm font-medium">深色模式</p>
            <p className="text-xs text-gray-400">降低夜间使用时的视觉刺激</p>
          </div>
          <button
            role="switch"
            aria-checked={theme === 'dark'}
            onClick={onToggleTheme}
            className={`relative w-11 h-6 rounded-full transition-colors ${
              theme === 'dark' ? 'bg-blue-500' : 'bg-gray-300'
            }`}
          >
            <span
              className={`absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow transition-transform ${
                theme === 'dark' ? 'translate-x-5' : ''
              }`}
            />
          </button>
        </div>

        {/* Custom app categories */}
        <div className="mt-4 flex-1 overflow-auto">
          <p className="text-sm font-medium mb-2">应用分类规则</p>
          <p className="text-xs text-gray-400 mb-3">
            自定义各类应用计入的类别，用于复盘看板的时间分布统计。
          </p>

          <div className="flex gap-2 mb-3">
            <input
              className="flex-1 px-2 py-1.5 text-sm border border-gray-300 rounded-md outline-none focus:ring-1 focus:ring-blue-400 dark:bg-gray-700 dark:border-gray-600 dark:text-gray-100"
              placeholder="应用名，如 Spotify"
              value={appInput}
              onChange={(e) => setAppInput(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleAdd()}
            />
            <select
              className="px-2 py-1.5 text-sm border border-gray-300 rounded-md outline-none focus:ring-1 focus:ring-blue-400 dark:bg-gray-700 dark:border-gray-600 dark:text-gray-100"
              value={categoryInput}
              onChange={(e) => setCategoryInput(e.target.value)}
            >
              {PRESET_CATEGORIES.map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
            <button
              className="px-3 py-1.5 text-sm bg-blue-500 text-white rounded-md hover:bg-blue-600 disabled:opacity-50"
              onClick={handleAdd}
              disabled={loading || !appInput.trim()}
            >
              添加
            </button>
          </div>

          {entries.length === 0 ? (
            <p className="text-xs text-gray-400 py-3 text-center">
              暂无自定义规则，将使用内置启发式分类。
            </p>
          ) : (
            <ul className="space-y-1">
              {entries.map(([app, cat]) => (
                <li
                  key={app}
                  className="flex items-center justify-between px-3 py-2 rounded-lg bg-gray-50 dark:bg-gray-700/60"
                >
                  <span className="text-sm truncate">{app}</span>
                  <div className="flex items-center gap-2">
                    <span className="text-xs text-blue-600 dark:text-blue-300 bg-blue-50 dark:bg-blue-900/40 px-2 py-0.5 rounded">
                      {cat}
                    </span>
                    <button
                      className="text-gray-300 hover:text-red-500"
                      onClick={() => handleDelete(app)}
                      aria-label={`删除 ${app} 的分类`}
                    >
                      <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12" />
                      </svg>
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="flex justify-end mt-4 pt-3 border-t border-gray-100 dark:border-gray-700">
          <button
            className="px-4 py-2 text-sm bg-gray-100 text-gray-600 hover:bg-gray-200 rounded-lg transition-colors dark:bg-gray-700 dark:text-gray-200"
            onClick={onClose}
          >
            完成
          </button>
        </div>
      </div>
    </div>
  );
}
