import {defineConfig} from 'vite'
import react, {reactCompilerPreset} from '@vitejs/plugin-react'
import babel from '@rolldown/plugin-babel'

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    react(),
    babel({presets: [reactCompilerPreset()]})
  ],
  build: {
    rollupOptions: {
      output: {
        entryFileNames: `assets/index.js`,
        chunkFileNames: `assets/[name].js`,
        assetFileNames: (assetInfo) => {
          // Use assetInfo.names[0] since name is deprecated in Rollup 4+
          const fileName = assetInfo.names?.[0] || '';
          if (fileName.endsWith('.css')) {
            return `assets/index.css`;
          }
          return `assets/[name].[ext]`;
        }
      }
    }
  }
})
