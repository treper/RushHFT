import { invoke } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';

export interface TriggerRule { rule_id: number; name: string; is_enabled: boolean; conditions: any[]; actions: any[]; }
export const triggers = writable<TriggerRule[]>([]);

export async function loadTriggers() { triggers.set(await invoke<TriggerRule[]>('get_triggers')); }
export async function saveTrigger(rule: TriggerRule) { await invoke('save_trigger', { rule }); await loadTriggers(); }
export async function deleteTrigger(id: number) { await invoke('delete_trigger', { ruleId: id }); await loadTriggers(); }
export async function testTrigger(id: number) { return invoke<string>('test_trigger_rest', { ruleId: id }); }
