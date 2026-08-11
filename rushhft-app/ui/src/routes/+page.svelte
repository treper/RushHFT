<script lang="ts">
  import { onMount } from 'svelte';
  import { currentSymbol, loadSymbols } from '$lib/stores/symbols';
  import { startPolling, stopPolling } from '$lib/stores/snapshot';
  import { subscribeNotifications } from '$lib/stores/notifications';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import DepthLadder from '$lib/components/DepthLadder.svelte';
  import TopOfBook from '$lib/components/TopOfBook.svelte';
  import TradesTape from '$lib/components/TradesTape.svelte';
  import Positions from '$lib/components/Positions.svelte';
  import CumulativeBook from '$lib/components/Charts/CumulativeBook.svelte';
  import PriceChart from '$lib/components/Charts/PriceChart.svelte';
  import PluginManagerModal from '$lib/modals/PluginManagerModal.svelte';
  import SettingsModal from '$lib/modals/SettingsModal.svelte';
  import TriggersModal from '$lib/modals/TriggersModal.svelte';
  import MultiVenueModal from '$lib/modals/MultiVenueModal.svelte';

  onMount(async () => {
    await loadSymbols();
    await subscribeNotifications().catch(() => {});
  });

  // Reactive polling: restart whenever the active symbol changes.
  $effect(() => {
    const sym = $currentSymbol;
    if (!sym) return;
    startPolling(sym);
    return () => stopPolling();
  });
</script>

<div class="app">
  <Sidebar />
  <main class="main">
    <TopOfBook />
    <CumulativeBook />
    <PriceChart />
    <div style="display:grid; grid-template-columns:1fr 1fr; gap:4px; min-height:0;">
      <DepthLadder />
      <TradesTape />
    </div>
    <Positions />
  </main>
</div>

<PluginManagerModal />
<SettingsModal />
<TriggersModal />
<MultiVenueModal />
