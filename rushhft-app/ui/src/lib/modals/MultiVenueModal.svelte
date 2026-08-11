<script lang="ts">
  import { openMultiVenue } from '$lib/components/events';
  import { invoke } from '@tauri-apps/api/core';
  import { currentSymbol } from '$lib/stores/symbols';

  interface VenuePrice { venue: string; bid: string; ask: string; last: string; timestamp: number; }
  let rows: VenuePrice[] = [];

  async function refresh() {
    try { rows = await invoke<VenuePrice[]>('get_multi_venue_prices', { symbol: $currentSymbol }); }
    catch { rows = []; }
  }
</script>

{#if $openMultiVenue}
  <div class="modal-backdrop" onclick={() => openMultiVenue.set(false)}>
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <h2>Multi-Venue Prices — {$currentSymbol}</h2>
      <button onclick={refresh}>Refresh</button>
      {#if rows.length === 0}
        <p style="color: var(--muted);">No other venues configured.</p>
      {:else}
        <table style="width:100%; font-family: ui-monospace, monospace;">
          <thead><tr><th>Venue</th><th>Bid</th><th>Ask</th><th>Last</th></tr></thead>
          <tbody>
            {#each rows as r}<tr><td>{r.venue}</td><td>{r.bid}</td><td>{r.ask}</td><td>{r.last}</td></tr>{/each}
          </tbody>
        </table>
      {/if}
    </div>
  </div>
{/if}
