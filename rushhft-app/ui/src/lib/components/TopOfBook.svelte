<script lang="ts">
  import { snapshot } from '$lib/stores/snapshot';
  let stale = $derived(!$snapshot || $snapshot.provider_status !== 'Connected');
  let bid = $derived($snapshot?.bids?.[0]);
  let ask = $derived($snapshot?.asks?.[0]);
</script>

<div class="panel">
  <div class="panel-header">Top of Book {#if stale}<span class="stale">stale</span>{/if}</div>
  <div class="tob">
    <span class="bid">{bid?.price ?? '-'} <span style="font-size:11px;">{bid?.size ?? ''}</span></span>
    <span style="color:var(--muted); font-size:11px;">mid {$snapshot?.mid_price ?? '-'}</span>
    <span class="ask">{ask?.price ?? '-'} <span style="font-size:11px;">{ask?.size ?? ''}</span></span>
  </div>
</div>
