<script lang="ts">
  import { providers } from '$lib/stores/snapshot';
</script>

<div>
  {#each $providers as p}
    <span class="provider">
      <span class="dot" style="background: {p.status === 'Connected' ? 'var(--bid)' : 'var(--muted)'};"></span>
      {p.name} — {p.status}
    </span>
  {/each}
</div>
