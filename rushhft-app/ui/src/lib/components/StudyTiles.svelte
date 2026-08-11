<script lang="ts">
  import { snapshot } from '$lib/stores/snapshot';
</script>

<div class="sidebar-scroll">
  {#each $snapshot?.studies ?? [] as s}
    <div class="tile" class:stale={s.is_stale}>
      <div class="label">{s.name}</div>
      <div class="value">{s.value}</div>
    </div>
  {/each}
  {#if ($snapshot?.studies ?? []).length === 0}
    <div class="empty-state">No studies running</div>
  {/if}
</div>
