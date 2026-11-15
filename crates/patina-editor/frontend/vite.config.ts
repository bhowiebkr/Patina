import { defineConfig } from 'vite';

// The page loads from the custom protocol of the Rust side, so the asset paths are relative.
export default defineConfig({
  base: './',
  build: { target: 'es2022' },
});
