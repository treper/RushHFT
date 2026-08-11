<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  let symbol = $state('700.HK');
  let snapshot = $state<any>(null);
  let providers = $state<any[]>([]);
  let studies = $state<any[]>([]);
  let stopped = false;

  async function poll() {
    while (!stopped) {
      try {
        const [snap, ps, sts] = await Promise.all([
          invoke('get_snapshot', { symbol }),
          invoke('get_providers'),
          invoke('get_studies'),
        ]);
        snapshot = snap;
        providers = ps;
        studies = sts;
      } catch (e) {
        // first failure is expected before plugin starts
      }
      await new Promise((r) => setTimeout(r, 500));
    }
  }

  onMount(() => {
    poll();
    return () => {
      stopped = true;
    };
  });
</script>

<header
  style="padding:8px 12px; border-bottom:1px solid var(--border); display:flex; gap:12px; align-items:center;"
>
  <strong style="color: var(--accent);">RushHFT</strong>
  <input
    bind:value={symbol}
    style="background:var(--panel); color:inherit; border:1px solid var(--border); padding:4px 8px; border-radius:4px;"
  />
  <span style="color: var(--muted);">Providers:</span>
  {#each providers as p}
    <span style="color: {p.status === 'Connected' ? 'var(--bid)' : 'var(--ask)'};">
      ● {p.name}
    </span>
  {/each}
</header>

<main
  style="display:grid; grid-template-columns: 220px 1fr 1fr; gap:6px; padding:6px; height: calc(100vh - 48px);"
>
  <section style="background:var(--panel); border:1px solid var(--border); padding:8px; overflow:auto;">
    <h3 style="margin:0 0 8px;">Asks</h3>
    {#each snapshot?.asks ?? [] as ask}
      <div style="display:flex; justify-content:space-between; color:var(--ask);">
        <span>{ask.price}</span>
        <span>{ask.size}</span>
      </div>
    {/each}
    <div style="border-top:1px solid var(--border); margin:8px 0; padding-top:8px;">
      <strong>Spread: {snapshot?.spread ?? '-'}</strong>
    </div>
    <h3 style="margin:0 0 8px;">Bids</h3>
    {#each snapshot?.bids ?? [] as bid}
      <div style="display:flex; justify-content:space-between; color:var(--bid);">
        <span>{bid.price}</span>
        <span>{bid.size}</span>
      </div>
    {/each}
  </section>

  <section style="background:var(--panel); border:1px solid var(--border); padding:8px; overflow:auto;">
    <h3 style="margin:0 0 8px;">Recent Trades</h3>
    {#each snapshot?.recent_trades ?? [] as t}
      <div
        style="display:grid; grid-template-columns:1fr 1fr 1fr; gap:8px; font-family:ui-monospace, monospace; font-size:12px;"
      >
        <span
          style="color: {t.direction === 'Up'
            ? 'var(--bid)'
            : t.direction === 'Down'
              ? 'var(--ask)'
              : 'var(--muted)'};"
        >
          {t.price}
        </span>
        <span>{t.size}</span>
        <span style="color:var(--muted);">{new Date(t.timestamp).toLocaleTimeString()}</span>
      </div>
    {/each}
  </section>

  <section style="background:var(--panel); border:1px solid var(--border); padding:8px; overflow:auto;">
    <h3 style="margin:0 0 8px;">Studies</h3>
    {#each snapshot?.studies ?? [] as s}
      <div style="display:flex; justify-content:space-between; padding:4px 0;">
        <span>{s.name}</span>
        <strong style="color:var(--accent);">{s.value}</strong>
      </div>
    {/each}
    <hr style="border-color:var(--border); margin:12px 0;" />
    <h3 style="margin:0 0 8px;">Plugins</h3>
    {#each studies as s}
      <div style="display:flex; justify-content:space-between; padding:2px 0;">
        <span>{s.name}</span>
        <span style="color: {s.status === 'Started' ? 'var(--bid)' : 'var(--muted)'};">
          {s.status}
        </span>
      </div>
    {/each}
  </section>
</main>
