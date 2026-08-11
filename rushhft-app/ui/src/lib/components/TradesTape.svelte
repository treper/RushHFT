<script lang="ts">
  import { snapshot } from '$lib/stores/snapshot';
  const fmt = (t: number) => new Date(t).toLocaleTimeString();
</script>

<div class="panel" style="display:flex; flex-direction:column;">
  <div class="panel-header">Recent Trades</div>
  <div style="overflow:auto; flex:1;">
    {#each $snapshot?.recent_trades ?? [] as t}
      <div class="trade">
        <span style="color: {t.direction === 'Up' ? 'var(--bid)' : t.direction === 'Down' ? 'var(--ask)' : 'var(--muted)'};">{t.price}</span>
        <span>{t.size}</span>
        <span style="color: var(--muted);">{fmt(t.timestamp)}</span>
      </div>
    {/each}
  </div>
</div>
