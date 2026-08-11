<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { currentSymbol } from '$lib/stores/symbols';
  import { fetchChartSeries } from '$lib/stores/snapshot';
  import { buildCumulativeOptions, cumulativeData } from '$lib/charts/series';
  import uPlot from 'uplot';

  let bidsEl: HTMLDivElement;
  let asksEl: HTMLDivElement;
  let bidsChart: uPlot | null = null;
  let asksChart: uPlot | null = null;
  let stopped = false;

  async function refresh() {
    const [b, a] = await Promise.all([
      fetchChartSeries($currentSymbol, 'cumulative-bids', 600),
      fetchChartSeries($currentSymbol, 'cumulative-asks', 600),
    ]);
    if (bidsChart && b.length) bidsChart.setData(cumulativeData(b));
    if (asksChart && a.length) asksChart.setData(cumulativeData(a));
  }

  onMount(async () => {
    bidsChart = new uPlot(buildCumulativeOptions(290, 120, 'Cum Bids', '#f85149'), [[]], bidsEl);
    asksChart = new uPlot(buildCumulativeOptions(290, 120, 'Cum Asks', '#7ee787'), [[]], asksEl);
    while (!stopped) { await refresh(); await new Promise((r) => setTimeout(r, 1000)); }
  });

  onDestroy(() => { stopped = true; bidsChart?.destroy(); asksChart?.destroy(); });
</script>

<div class="panel">
  <div class="panel-header">Cumulative Book</div>
  <div style="display:flex; gap:4px;">
    <div bind:this={bidsEl}></div>
    <div bind:this={asksEl}></div>
  </div>
</div>
