import { invoke } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';

export interface PluginDescriptor {
  plugin_id: string;
  name: string;
  version: string;
  description: string;
  plugin_type: string;
  status: string;
  emits_metric: boolean;
}

export const plugins = writable<PluginDescriptor[]>([]);

export async function loadPlugins() {
  // Fall back to get_studies (older command) if get_plugin_descriptors errors.
  try {
    plugins.set(await invoke<PluginDescriptor[]>('get_plugin_descriptors'));
  } catch {
    plugins.set(await invoke<any[]>('get_studies'));
  }
}

export async function startPlugin(id: string) {
  await invoke('start_plugin', { pluginId: id });
  await loadPlugins();
}

export async function stopPlugin(id: string) {
  await invoke('stop_plugin', { pluginId: id });
  await loadPlugins();
}
