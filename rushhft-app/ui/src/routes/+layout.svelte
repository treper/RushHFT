<script lang="ts">
  import '../app.css';

  let { children } = $props();
</script>

{@render children()}

<style>
  :global(body) {
    margin: 0;
    background: #0d1117;
    color: #c9d1d9;
    font-family: -apple-system, system-ui, sans-serif;
  }
</style>
