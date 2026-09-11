/**
 * vite.config.ts
 *
 * Vite build configuration for the PCB Forge webview React application.
 *
 * Key decisions:
 * - Uses @vitejs/plugin-react for JSX transform and Fast Refresh
 * - Uses @rolldown/plugin-babel with the React Compiler preset to enable
 *   automatic memoisation without manual useMemo / useCallback calls
 * - Forces deterministic output filenames (assets/index.js and assets/index.css)
 *   so that extension.ts can reference them by a fixed path regardless of
 *   content hashing that Vite normally applies
 */

import {defineConfig} from 'vite'
import react, {reactCompilerPreset} from '@vitejs/plugin-react'
import babel from '@rolldown/plugin-babel'

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    react(),
    // Run the React Compiler (Forget) via Babel to automatically optimise
    // re-renders without manual memoisation
    babel({presets: [reactCompilerPreset()]})
  ],
  build: {
    rollupOptions: {
      output: {
        // Fixed entry-point names so extension.ts can reference them without
        // knowing the content hash that Vite/Rollup would otherwise append
        entryFileNames: `assets/index.js`,
        chunkFileNames: `assets/[name].js`,
        assetFileNames: (assetInfo) => {
          // Use assetInfo.names[0] since `name` is deprecated in Rollup 4+
          const fileName = assetInfo.names?.[0] || '';
          if (fileName.endsWith('.css')) {
            // Keep CSS at a fixed path for the same reason as JS above
            return `assets/index.css`;
          }
          return `assets/[name].[ext]`;
        }
      }
    }
  }
})
