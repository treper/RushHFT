<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { currentSymbol, loadSymbols, addSymbol, removeSymbol } from '$lib/stores/symbols';
  import { startPolling, stopPolling } from '$lib/stores/snapshot';
  import { subscribeNotifications } from '$lib/stores/notifications';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import DepthLadder from '$lib/components/DepthLadder.svelte';
  import TopOfBook from '$lib/components/TopOfBook.svelte';
  import LOBImbalanceGauge from '$lib/components/LOBImbalanceGauge.svelte';
  import TradesTape from '$lib/components/TradesTape.svelte';
  import Positions from '$lib/components/Positions.svelte';
  import CumulativeBook from '$lib/components/Charts/CumulativeBook.svelte';
  import PriceChart from '$lib/components/Charts/PriceChart.svelte';
  import SpreadChart from '$lib/components/Charts/SpreadChart.svelte';
  import PluginManagerModal from '$lib/modals/PluginManagerModal.svelte';
  import SettingsModal from '$lib/modals/SettingsModal.svelte';
  import TriggersModal from '$lib/modals/TriggersModal.svelte';
  import MultiVenueModal from '$lib/modals/MultiVenueModal.svelte';

  let newSymbol = $state('');

  onMount(async () => {
    await loadSymbols();
    await startPolling($currentSymbol);
    await subscribeNotifications().catch(() => {});
  });

  onDestroy(() => stopPolling());

  async function onAdd() {
    if (!newSymbol) return;
    await addSymbol(newSymbol);
    newSymbol = '';
  }
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
