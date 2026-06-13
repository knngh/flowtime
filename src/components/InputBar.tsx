import { useState, useRef, KeyboardEvent } from 'react';
import { createTask } from '../db';

interface InputBarProps {
  activeProjectId: string | null;
  onCreated: () => void;
  onNlSubmit: (input: string) => void;
  isParsing: boolean;
  parseError: string | null;
  calibrationRatio?: number;
}

export default function InputBar({
  activeProjectId,
  onCreated,
  onNlSubmit,
  isParsing,
  parseError,
  calibrationRatio = 1.0,
}: InputBarProps) {
  const [input, setInput] = useState('');
  const [mode, setMode] = useState<'quick' | 'nl'>('nl');
  const [quickPriority, setQuickPriority] = useState<'A' | 'B' | 'C'>('B');
  const [quickDuration, setQuickDuration] = useState(30);
  const inputRef = useRef<HTMLInputElement>(null);

  const handleCreate = async () => {
    const title = input.trim();
    if (!title) return;
    await createTask(title, activeProjectId, quickPriority, quickDuration);
    setInput('');
    onCreated();
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      const trimmed = input.trim();
      if (!trimmed) return;

      if (mode === 'nl') {
        onNlSubmit(trimmed);
        setInput('');
      } else {
        handleCreate();
      }
    }
  };

  return (
    <div className="border-t border-gray-100 bg-white px-4 py-3">
      {parseError && (
        <div className="mb-2 px-3 py-1.5 bg-amber-50 border border-amber-200 rounded-md text-xs text-amber-700">
          AI 解析失败（使用本地规则兜底），可手动调整后确认
        </div>
      )}

      {/* Mode toggle */}
      <div className="flex items-center gap-2 mb-2">
        <button
          className={`text-xs px-2 py-0.5 rounded transition-colors ${
            mode === 'nl'
              ? 'bg-purple-100 text-purple-700 font-medium'
              : 'text-gray-400 hover:text-gray-600'
          }`}
          onClick={() => setMode('nl')}
        >
          AI 识别
        </button>
        <button
          className={`text-xs px-2 py-0.5 rounded transition-colors ${
            mode === 'quick'
              ? 'bg-blue-100 text-blue-700 font-medium'
              : 'text-gray-400 hover:text-gray-600'
          }`}
          onClick={() => setMode('quick')}
        >
          快速创建
        </button>
        {isParsing && (
          <span className="text-xs text-purple-500 ml-auto animate-pulse">正在解析…</span>
        )}
        {/* M8: Calibration hint */}
        {calibrationRatio !== 1.0 && mode === 'quick' && (
          <span className="text-xs text-amber-600 ml-auto" title={`历史校准系数: ${calibrationRatio.toFixed(2)}`}>
            建议时长 x{calibrationRatio.toFixed(1)}
          </span>
        )}
      </div>

      <div className="flex items-center gap-2">
        <div className="flex-1 relative">
          <input
            ref={inputRef}
            className="w-full px-4 py-2 text-sm border border-gray-200 rounded-lg outline-none focus:ring-2 focus:ring-purple-200 focus:border-purple-400 transition-colors disabled:bg-gray-50"
            placeholder={
              mode === 'nl'
                ? '自然语言输入，回车解析…（例：下午花2小时写支付API，优先级A）'
                : '输入任务名称，回车创建…'
            }
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            disabled={isParsing}
          />
          {isParsing && (
            <div className="absolute right-3 top-1/2 -translate-y-1/2">
              <div className="w-4 h-4 border-2 border-purple-300 border-t-purple-600 rounded-full animate-spin" />
            </div>
          )}
        </div>

        {mode === 'quick' && (
          <>
            <select
              className="text-xs border border-gray-200 rounded-md px-1.5 py-2 outline-none focus:ring-1 focus:ring-blue-200"
              value={quickPriority}
              onChange={(e) => setQuickPriority(e.target.value as 'A' | 'B' | 'C')}
              title="优先级"
            >
              <option value="A">A 高</option>
              <option value="B">B 中</option>
              <option value="C">C 低</option>
            </select>

            <select
              className="text-xs border border-gray-200 rounded-md px-1.5 py-2 outline-none focus:ring-1 focus:ring-blue-200"
              value={quickDuration}
              onChange={(e) => setQuickDuration(Number(e.target.value))}
              title="预计时长"
            >
              <option value={15}>15分钟</option>
              <option value={30}>30分钟</option>
              <option value={60}>1小时</option>
              <option value={90}>1.5小时</option>
              <option value={120}>2小时</option>
            </select>

            <button
              className="px-3 py-2 bg-blue-500 text-white text-sm rounded-lg hover:bg-blue-600 transition-colors disabled:opacity-40"
              disabled={!input.trim()}
              onClick={handleCreate}
            >
              创建
            </button>
          </>
        )}
      </div>
    </div>
  );
}