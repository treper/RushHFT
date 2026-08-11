<script lang="ts">
  import Toolbar from './Toolbar.svelte';
  import ProviderStatus from './ProviderStatus.svelte';
  import StudyTiles from './StudyTiles.svelte';
</script>

<aside class="sidebar">
  <Toolbar />
  <div style="padding:6px 8px; border-bottom:1px solid var(--border);">
    <ProviderStatus />
  </div>
  <StudyTiles />
</aside>
