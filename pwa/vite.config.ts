import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// https://vite.dev/config/
export default defineConfig({
  base: './',
  plugins: [
    react(),
    tailwindcss()
  ],
  build: {
    target: 'es2020',
    minify: 'terser',
    terserOptions: {
      safari10: true,
    },
    cssTarget: 'safari15'
  }
})
