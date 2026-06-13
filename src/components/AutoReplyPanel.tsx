import { useState } from 'react';
import type { PendingReply } from '../auto_reply';
import { updateReplyDraft, markReplySent, discardReply } from '../auto_reply';

interface AutoReplyPanelProps {
  replies: PendingReply[];
  onRefresh: () => void;
}

export default function AutoReplyPanel({ replies, onRefresh }: AutoReplyPanelProps) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editText, setEditText] = useState('');
  const [actionStatus, setActionStatus] = useState<string | null>(null);

  const startEditing = (reply: PendingReply) => {
    setEditingId(reply.id);
    setEditText(reply.reply_draft);
  };

  const saveEdit = async (replyId: string) => {
    try {
      await updateReplyDraft(replyId, editText);
      setEditingId(null);
      setActionStatus('已保存');
      onRefresh();
    } catch {
      setActionStatus('保存失败');
    }
  };

  const handleCopy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setActionStatus('已复制到剪贴板');
    } catch {
      setActionStatus('复制失败');
    }
    setTimeout(() => setActionStatus(null), 2000);
  };

  const handleSend = async (replyId: string, text: string) => {
    await navigator.clipboard.writeText(text);
    await markReplySent(replyId);
    setActionStatus('已复制并标记为已发送');
    setTimeout(() => setActionStatus(null), 2000);
    onRefresh();
  };

  const handleDiscard = async (replyId: string) => {
    await discardReply(replyId);
    setActionStatus('已丢弃');
    setTimeout(() => setActionStatus(null), 2000);
    onRefresh();
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h4 className="text-xs font-semibold text-gray-500 uppercase tracking-wider">
          AI 回复草稿 ({replies.length})
        </h4>
        {actionStatus && (
          <span className="text-[11px] text-green-600 bg-green-50 px-2 py-0.5 rounded-full">
            {actionStatus}
          </span>
        )}
      </div>

      {replies.length === 0 ? (
        <p className="text-xs text-gray-400 py-2">暂无待处理的回复草稿</p>
      ) : (
        <div className="space-y-2">
          {replies.map((reply) => (
            <div
              key={reply.id}
              className="bg-gray-50 rounded-lg p-3 border border-gray-100"
            >
              <div className="flex items-center justify-between mb-1.5">
                <span className="text-[10px] text-gray-400 uppercase">
                  {reply.channel}
                </span>
                <span className="text-[10px] text-gray-300">
                  {new Date(reply.created_at).toLocaleTimeString('zh-CN', {
                    hour: '2-digit',
                    minute: '2-digit',
                  })}
                </span>
              </div>

              <p className="text-xs text-gray-600 mb-2 italic border-l-2 border-gray-200 pl-2">
                &ldquo;{reply.original_message.slice(0, 80)}{reply.original_message.length > 80 ? '…' : ''}&rdquo;
              </p>

              {editingId === reply.id ? (
                <div>
                  <textarea
                    className="w-full text-xs border border-blue-300 rounded p-2 mb-2 focus:outline-none focus:ring-2 focus:ring-blue-200 resize-none"
                    rows={3}
                    value={editText}
                    onChange={(e) => setEditText(e.target.value)}
                  />
                  <div className="flex gap-1.5">
                    <button
                      className="text-[11px] px-2.5 py-1 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
                      onClick={() => saveEdit(reply.id)}
                    >
                      保存
                    </button>
                    <button
                      className="text-[11px] px-2.5 py-1 text-gray-500 hover:bg-gray-200 rounded transition-colors"
                      onClick={() => setEditingId(null)}
                    >
                      取消
                    </button>
                  </div>
                </div>
              ) : (
                <div>
                  <p className="text-xs text-gray-800 mb-2 bg-white rounded p-2 border border-gray-100">
                    {reply.reply_draft}
                  </p>
                  <div className="flex gap-1.5">
                    <button
                      className="text-[11px] px-2.5 py-1 bg-gray-700 text-white rounded hover:bg-gray-800 transition-colors"
                      onClick={() => handleSend(reply.id, reply.reply_draft)}
                    >
                      发送（复制）
                    </button>
                    <button
                      className="text-[11px] px-2.5 py-1 text-gray-500 hover:bg-gray-200 rounded transition-colors"
                      onClick={() => handleCopy(reply.reply_draft)}
                    >
                      复制
                    </button>
                    <button
                      className="text-[11px] px-2.5 py-1 text-blue-500 hover:bg-blue-50 rounded transition-colors"
                      onClick={() => startEditing(reply)}
                    >
                      编辑
                    </button>
                    <button
                      className="text-[11px] px-2.5 py-1 text-red-400 hover:bg-red-50 rounded transition-colors"
                      onClick={() => handleDiscard(reply.id)}
                    >
                      丢弃
                    </button>
                  </div>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}