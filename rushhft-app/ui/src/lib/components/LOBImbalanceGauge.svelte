<script lang="ts">
  import { snapshot } from '$lib/stores/snapshot';

  // Find LOB Imbalance study value (range -1..+1).
  let imb = $derived(
    (() => {
      const s = ($snapshot?.studies ?? []).find((x) => x.name === 'Imbalance');
      return s ? Number(s.value) : 0;
    })(),
  );
  // Map [-1, +1] -> [0%, 100%]; 0 = center.
  let pct = $derived(50 + imb * 50);
</script>

<div class="panel">
  <div class="panel-header">LOB Imbalance</div>
  <div style="padding:8px;">
    <div class="gauge">
      <div class="arrow" style="left: {pct}%;"></div>
    </div>
    <div style="display:flex; justify-content:space-between; font-size:10px; color:var(--muted); margin-top:4px;">
      <span>bids</span><span>{imb.toFixed(3)}</span><span>asks</span>
    </div>
  </div>
</div>
