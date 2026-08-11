<script lang="ts">
  import { symbols, currentSymbol, addSymbol, removeSymbol } from '$lib/stores/symbols';

  let input = $state('');

  async function onAdd() {
    const sym = input.trim().toUpperCase();
    if (!sym) return;
    try {
      await addSymbol(sym);
      currentSymbol.set(sym);
    } catch (e) {
      console.error('subscribe failed:', e);
    }
    input = '';
  }

  async function onRemove(sym: string) {
    const wasCurrent = $currentSymbol === sym;
    await removeSymbol(sym);
    if (wasCurrent) {
      const remaining = $symbols.filter((s) => s !== sym);
      if (remaining.length > 0) {
        currentSymbol.set(remaining[0]);
      }
    }
  }

  function select(sym: string) {
    currentSymbol.set(sym);
  }
</script>

<section class="symbol-manager">
  <form class="symbol-form" onsubmit={(e) => { e.preventDefault(); onAdd(); }}>
    <input
      class="symbol-input"
      bind:value={input}
      placeholder="e.g. 700.HK, AAPL.US"
      autocomplete="off"
    />
    <button type="submit" class="symbol-add-btn">Add</button>
  </form>
  <div class="symbol-list">
    {#each $symbols as sym (sym)}
      <div class="symbol-row" class:active={sym === $currentSymbol}>
        <button type="button" class="symbol-btn" onclick={() => select(sym)}>
          {sym}
        </button>
        <button
          type="button"
          class="symbol-remove"
          onclick={() => onRemove(sym)}
          aria-label="Remove {sym}"
        >
          ×
        </button>
      </div>
    {:else}
      <div class="symbol-empty">No symbols subscribed</div>
    {/each}
  </div>
</section>

<style>
  .symbol-manager {
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
    background: var(--panel-2);
  }
  .symbol-form {
    display: flex;
    gap: 4px;
    margin-bottom: 6px;
  }
  .symbol-input {
    flex: 1;
    background: var(--bg);
    color: inherit;
    border: 1px solid var(--border);
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 11px;
    font-family: ui-monospace, "SF Mono", monospace;
    min-width: 0;
  }
  .symbol-input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .symbol-add-btn {
    background: var(--panel);
    color: inherit;
    border: 1px solid var(--border);
    padding: 4px 10px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 11px;
    white-space: nowrap;
  }
  .symbol-add-btn:hover {
    border-color: var(--accent);
  }
  .symbol-list {
    max-height: 140px;
    overflow-y: auto;
  }
  .symbol-row {
    display: flex;
    align-items: center;
    border-radius: 4px;
    margin-bottom: 2px;
  }
  .symbol-row:hover {
    background: var(--panel);
  }
  .symbol-row.active {
    background: var(--panel);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .symbol-btn {
    flex: 1;
    background: transparent;
    border: none;
    color: inherit;
    padding: 3px 8px;
    cursor: pointer;
    text-align: left;
    font-size: 11px;
    font-family: ui-monospace, "SF Mono", monospace;
  }
  .symbol-row.active .symbol-btn {
    color: var(--accent);
    font-weight: 600;
  }
  .symbol-remove {
    background: transparent;
    border: none;
    color: var(--muted);
    cursor: pointer;
    padding: 3px 8px;
    font-size: 14px;
    line-height: 1;
  }
  .symbol-remove:hover {
    color: var(--err);
  }
  .symbol-empty {
    color: var(--muted);
    font-size: 10px;
    padding: 4px 8px;
  }
</style>
