<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { currentSymbol } from '$lib/stores/symbols';
  import { fetchChartSeries } from '$lib/stores/snapshot';
  import { buildSpreadOptions, spreadData } from '$lib/charts/series';
  import uPlot from 'uplot';

  let container: HTMLDivElement;
  let chart: uPlot | null = null;
  let stopped = false;

  async function refresh() {
    const pts = await fetchChartSeries($currentSymbol, 'spread', 600);
    if (chart && pts.length) chart.setData(spreadData(pts));
  }

  onMount(async () => {
    chart = new uPlot(buildSpreadOptions(600, 120), [[]], container);
    while (!stopped) {
      await refresh();
      await new Promise((r) => setTimeout(r, 1000));
    }
  });

  onDestroy(() => { stopped = true; chart?.destroy(); });
</script>

<div class="panel">
  <div class="panel-header">Spread</div>
  <div bind:this={container}></div>
</div>
