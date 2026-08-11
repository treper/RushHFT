<script lang="ts">
  import { openPluginManager } from '$lib/components/events';
  import { plugins, loadPlugins, startPlugin, stopPlugin } from '$lib/stores/plugins';
  import { onMount } from 'svelte';
  onMount(loadPlugins);
</script>

{#if $openPluginManager}
  <div class="modal-backdrop" onclick={() => openPluginManager.set(false)}>
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <h2>Plugins</h2>
      {#each $plugins as p}
        <div class="row">
          <span>{p.name} <small style="color:var(--muted);">v{p.version}</small></span>
          <span>
            <small style="color: {p.status === 'Started' ? 'var(--bid)' : 'var(--muted)'};">{p.status}</small>
            {#if p.status === 'Started'}
              <button onclick={() => stopPlugin(p.plugin_id)}>Stop</button>
            {:else}
              <button onclick={() => startPlugin(p.plugin_id)}>Start</button>
            {/if}
          </span>
        </div>
      {/each}
    </div>
  </div>
{/if}
