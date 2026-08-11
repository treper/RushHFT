import { sveltekit } from '@sveltejs/vite-plugin-sveltekit';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [sveltekit()],
  server: {
    port: 5173,
    strictPort: true,
  },
});
