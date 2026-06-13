import { invoke } from '@tauri-apps/api/core';

export interface PendingReply {
  id: string;
  original_message: string;
  reply_draft: string;
  channel: string;
  created_at: string;
  status: string; // "pending" | "sent" | "discarded"
}

export async function generateAutoReply(
  originalMessage: string,
  channel: string,
): Promise<PendingReply> {
  return invoke<PendingReply>('generate_auto_reply', {
    originalMessage,
    channel,
  });
}

export async function getPendingReplies(): Promise<PendingReply[]> {
  return invoke<PendingReply[]>('get_pending_replies');
}

export async function updateReplyDraft(replyId: string, newDraft: string): Promise<void> {
  return invoke<void>('update_reply_draft', { replyId, newDraft });
}

export async function markReplySent(replyId: string): Promise<void> {
  return invoke<void>('mark_reply_sent', { replyId });
}

export async function discardReply(replyId: string): Promise<void> {
  return invoke<void>('discard_reply', { replyId });
}