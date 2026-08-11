<script lang="ts">
  import { snapshot } from '$lib/stores/snapshot';

  // Compute max size across both sides for bar scaling.
  let maxSize = $derived(
    Math.max(
      ...($snapshot?.bids ?? []).map((b) => Number(b.size)),
      ...($snapshot?.asks ?? []).map((a) => Number(a.size)),
      1,
    ),
  );

  // Asks descending (best ask first), bids descending (best bid first).
  let asks = $derived(($snapshot?.asks ?? []).slice().reverse());
  let bids = $derived($snapshot?.bids ?? []);
</script>

<div class="panel" style="display:flex; flex-direction:column; min-height:0;">
  <div class="panel-header">Depth — {$snapshot?.symbol ?? ''}</div>
  <div class="depth" style="overflow:auto; flex:1;">
    {#each asks as a}
      <div class="row ask">
        <div class="bar" style="width: {(Number(a.size) / maxSize) * 100}%;"></div>
        <span class="text">{a.price}</span>
        <span class="text">{a.size}</span>
      </div>
    {/each}
    <div class="spread">spread {$snapshot?.spread ?? '-'}</div>
    {#each bids as b}
      <div class="row bid">
        <div class="bar" style="width: {(Number(b.size) / maxSize) * 100}%;"></div>
        <span class="text">{b.price}</span>
        <span class="text">{b.size}</span>
      </div>
    {/each}
  </div>
</div>
