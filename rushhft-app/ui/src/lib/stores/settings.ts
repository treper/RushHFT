import { invoke } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';

export interface Settings {
  app_key: string;
  app_secret_masked: string;
  access_token_masked: string;
  default_symbols: string[];
  depth_levels: number;
  aggregation_level: string;
  log_level: string;
  region: string;
}

export const settings = writable<Settings | null>(null);

export async function loadSettings() {
  settings.set(await invoke<Settings>('get_settings'));
}

export async function saveSettings(s: Settings) {
  await invoke('save_settings', { settings: s });
  await loadSettings();
}
