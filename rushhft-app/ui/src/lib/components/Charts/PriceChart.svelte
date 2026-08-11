<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { currentSymbol } from '$lib/stores/symbols';
  import { fetchChartSeries } from '$lib/stores/snapshot';
  import { buildPriceOptions, priceData } from '$lib/charts/series';
  import uPlot from 'uplot';

  let container: HTMLDivElement;
  let chart: uPlot | null = null;
  let stopped = false;

  async function refresh() {
    const pts = await fetchChartSeries($currentSymbol, 'price', 600);
    if (chart && pts.length) chart.setData(priceData(pts));
  }

  onMount(async () => {
    chart = new uPlot(buildPriceOptions(600, 160), [[], [], [], []], container);
    while (!stopped) { await refresh(); await new Promise((r) => setTimeout(r, 1000)); }
  });

  onDestroy(() => { stopped = true; chart?.destroy(); });
</script>

<div class="panel">
  <div class="panel-header">Real-time Price</div>
  <div bind:this={container}></div>
</div>
