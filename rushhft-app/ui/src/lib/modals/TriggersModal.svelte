<script lang="ts">
  import { openTriggers } from '$lib/components/events';
  import { triggers, loadTriggers, saveTrigger, deleteTrigger, testTrigger } from '$lib/stores/triggers';
  import { onMount } from 'svelte';
  onMount(loadTriggers);
</script>

{#if $openTriggers}
  <div class="modal-backdrop" onclick={() => openTriggers.set(false)}>
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <h2>Triggers</h2>
      {#each $triggers as t}
        <div class="row">
          <span>{t.name} <small style="color: var(--muted);">(#{t.rule_id})</small></span>
          <span>
            <button onclick={async () => { await testTrigger(t.rule_id).catch(() => 'error'); }}>Test</button>
            <button onclick={async () => { await deleteTrigger(t.rule_id); }}>Delete</button>
          </span>
        </div>
      {/each}
    </div>
  </div>
{/if}
