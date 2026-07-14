import { useState, useEffect, useRef } from 'react';

interface FocusModeProps {
  sessionId: string;
  taskId: string | null;
  taskTitle: string | null;
  startTime: string;
  status: string;
  onEnd: () => void;
  onPause: () => void;
  onResume: () => void;
}

function formatDuration(totalSeconds: number): string {
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  const s = totalSeconds % 60;
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

export default function FocusMode({
  taskTitle,
  startTime,
  status,
  onEnd,
  onPause,
  onResume,
}: FocusModeProps) {
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const baseRef = useRef(0); // seconds accumulated before the current active period
  const elapsedRef = useRef(0);
  const periodStartRef = useRef(Date.now());

  useEffect(() => {
    if (status === 'active') {
      periodStartRef.current = Date.now();
      const id = setInterval(() => {
        const secs =
          baseRef.current + Math.floor((Date.now() - periodStartRef.current) / 1000);
        elapsedRef.current = secs;
        setElapsedSeconds(secs);
      }, 1000);
      setElapsedSeconds(baseRef.current);
      return () => clearInterval(id);
    }
    // paused: freeze the displayed elapsed time
    baseRef.current = elapsedRef.current;
  }, [status, startTime]);

  // Esc ends the session
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onEnd();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onEnd]);

  const isPaused = status === 'paused';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
      <div className="flex flex-col items-center gap-6 text-white">
        <div className="text-center">
          <p className="text-sm text-white/50 mb-1 tracking-widest uppercase">
            {isPaused ? '已暂停' : '专注中'}
          </p>
          <h1 className="text-2xl font-semibold max-w-md truncate">
            {taskTitle || '无任务'}
          </h1>
        </div>

        <div className="text-7xl font-mono font-light tracking-wider tabular-nums">
          {formatDuration(elapsedSeconds)}
        </div>

        <div className="flex items-center gap-3">
          {isPaused ? (
            <button
              className="px-6 py-3 bg-green-500/90 hover:bg-green-500 text-white rounded-xl text-base font-medium transition-all"
              onClick={onResume}
            >
              继续专注
            </button>
          ) : (
            <button
              className="px-6 py-3 bg-white/15 hover:bg-white/25 text-white rounded-xl text-base font-medium border border-white/20 transition-all"
              onClick={onPause}
            >
              暂停
            </button>
          )}
          <button
            className="px-8 py-3 bg-white/15 hover:bg-white/25 text-white rounded-xl text-base font-medium border border-white/20 transition-all backdrop-blur"
            onClick={onEnd}
          >
            结束专注
          </button>
        </div>

        <p className="text-xs text-white/30 mt-2">按 Esc 也可结束</p>
      </div>
    </div>
  );
}

export { formatDuration };
