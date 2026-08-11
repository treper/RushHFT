<script lang="ts">
  import { openSettings } from '$lib/components/events';
  import { settings, loadSettings, saveSettings } from '$lib/stores/settings';
  import { onMount } from 'svelte';
  onMount(loadSettings);
  let form = $state<any>({});
  $effect(() => { if ($settings) form = { ...$settings }; });
</script>

{#if $openSettings}
  <div class="modal-backdrop" onclick={() => openSettings.set(false)}>
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <h2>Settings</h2>
      <label>App Key</label><input bind:value={form.app_key} style="width:100%;" />
      <label>App Secret (leave masked to keep)</label><input bind:value={form.app_secret_masked} style="width:100%;" />
      <label>Access Token</label><input bind:value={form.access_token_masked} style="width:100%;" />
      <label>Default Symbols (comma-separated)</label>
      <input bind:value={form.default_symbols_input} placeholder="700.HK,AAPL.US" style="width:100%;" />
      <label>Region</label><input bind:value={form.region} />
      <label>Depth Levels</label><input type="number" bind:value={form.depth_levels} />
      <label>Log Level</label><input bind:value={form.log_level} />
      <div style="margin-top:12px; text-align:right;">
        <button onclick={async () => {
          const toSave = { ...form, default_symbols: form.default_symbols_input?.split(',').map((s:string)=>s.trim()).filter(Boolean) ?? [] };
          await saveSettings(toSave);
          openSettings.set(false);
        }}>Save</button>
      </div>
    </div>
  </div>
{/if}
