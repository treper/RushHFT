import { invoke } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';

export interface BookItem { price: string; size: string; cumulative_size: string; is_bid: boolean; broker_ids: number[]; }
export interface Trade { price: string; size: string; timestamp: number; direction: 'Neutral'|'Down'|'Up'; trade_type: string; }
export interface StudyValue { name: string; value: string; format: string; value_color: string; tooltip: string; has_error: boolean; is_stale: boolean; timestamp: number; }
export interface QuoteStats { last_done: string; open: string; high: string; low: string; volume: number; turnover: string; trade_status: string; timestamp: number; }
export interface Provider { id: number; name: string; status: string; }
export interface Snapshot {
  symbol: string;
  bids: BookItem[];
  asks: BookItem[];
  spread: string;
  mid_price: string;
  last_updated: number;
  sequence: number;
  provider_status: string;
  studies: StudyValue[];
  recent_trades: Trade[];
  quote_stats: QuoteStats | null;
}

export const snapshot = writable<Snapshot | null>(null);
export const providers = writable<Provider[]>([]);
export const chartSeries = writable<Record<string, any[]>>({});

let pollHandle: number | null = null;

export async function startPolling(symbol: string) {
  stopPolling();
  pollHandle = window.setInterval(async () => {
    try {
      const [snap, ps] = await Promise.all([
        invoke<Snapshot>('get_snapshot', { symbol }),
        invoke<Provider[]>('get_providers'),
      ]);
      snapshot.set(snap);
      providers.set(ps);
    } catch { /* plugin not started yet */ }
  }, 500);
}

export function stopPolling() {
  if (pollHandle !== null) { clearInterval(pollHandle); pollHandle = null; }
}

export async function fetchChartSeries(symbol: string, kind: string, points = 600): Promise<any[]> {
  try {
    const dto = await invoke<{ kind: string; points: any[] }>('get_chart_series', { symbol, kind, points });
    return dto.points;
  } catch { return []; }
}
