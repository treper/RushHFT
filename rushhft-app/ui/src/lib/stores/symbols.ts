import { invoke } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';

export const symbols = writable<string[]>([]);
export const currentSymbol = writable<string>('700.HK');
export const userSymbols = writable<string[]>([]);

export async function loadSymbols() {
  symbols.set(await invoke<string[]>('get_symbols'));
}

export async function addSymbol(symbol: string) {
  await invoke('add_symbol', { symbol });
  await loadSymbols();
}

export async function removeSymbol(symbol: string) {
  await invoke('remove_symbol', { symbol });
  await loadSymbols();
}
