import { useState, useEffect, useRef } from 'react';
import type { FocusSessionSummary } from '../types';

interface FocusModeProps {
  taskTitle: string | null;
  startTime: string;
  onEnd: (summary: FocusSessionSummary) => void;
}

function formatDuration(totalSeconds: number): string {
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  const s = totalSeconds % 60;
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

export default function FocusMode({ taskTitle, startTime, onEnd }: FocusModeProps) {
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const timerRef = useRef<ReturnType<typeof setInterval>>(null);

  useEffect(() => {
    const startMs = new Date(startTime).getTime();
    const tick = () => {
      setElapsedSeconds(Math.floor((Date.now() - startMs) / 1000));
    };
    tick();
    timerRef.current = setInterval(tick, 1000);
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, [startTime]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
      <div className="flex flex-col items-center gap-6 text-white">
        {/* Task name */}
        <div className="text-center">
          <p className="text-sm text-white/50 mb-1 tracking-widest uppercase">专注中</p>
          <h1 className="text-2xl font-semibold max-w-md truncate">
            {taskTitle || '无任务'}
          </h1>
        </div>

        {/* Timer */}
        <div className="text-7xl font-mono font-light tracking-wider tabular-nums">
          {formatDuration(elapsedSeconds)}
        </div>

        {/* End button */}
        <button
          className="mt-4 px-8 py-3 bg-white/15 hover:bg-white/25 text-white rounded-xl text-base font-medium border border-white/20 transition-all backdrop-blur"
          onClick={() => {
            onEnd({
              session_id: '',
              task_id: null,
              duration_seconds: elapsedSeconds,
              interruptions_blocked: 0,
              messages_auto_replied: 0,
            });
          }}
        >
          结束专注
        </button>

        <p className="text-xs text-white/30 mt-2">按 Esc 也可结束</p>
      </div>
    </div>
  );
}

export { formatDuration };