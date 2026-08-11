import { invoke } from '@tauri-apps/api/core';
import { Channel } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';

export interface Notification { source: string; message: string; level: string; category: string; timestamp: number; exception: string | null; }
export const notifications = writable<Notification[]>([]);
export const unreadCount = writable<number>(0);

export async function subscribeNotifications() {
  const ch = new Channel<Notification>();
  ch.onmessage = (n) => {
    notifications.update((list) => [...list.slice(-200), n]);
    unreadCount.update((c) => c + 1);
  };
  await invoke('subscribe_notifications', { channel: ch });
}

export function clearUnread() { unreadCount.set(0); }
