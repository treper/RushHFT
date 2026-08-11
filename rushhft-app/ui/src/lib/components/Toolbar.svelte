<script lang="ts">
  import { unreadCount, clearUnread } from '$lib/stores/notifications';
  import { openPluginManager, openSettings, openTriggers, openMultiVenue } from './events';

  let bellOpen = $state(false);
</script>

<div class="toolbar">
  <button onclick={() => openPluginManager.set(true)}>Plugins</button>
  <button onclick={() => openSettings.set(true)}>Settings</button>
  <button onclick={() => openTriggers.set(true)}>Triggers</button>
  <button onclick={() => openMultiVenue.set(true)}>MultiVenue</button>
  <button class="bell" onclick={() => { bellOpen = !bellOpen; if (bellOpen) clearUnread(); }}>
    Bell ({$unreadCount})
  </button>
</div>
