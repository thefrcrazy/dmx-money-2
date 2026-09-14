import { defineConfig, type Plugin } from 'vite'
import react from '@vitejs/plugin-react'
import legacy from '@vitejs/plugin-legacy'

function tauriLegacySystemJsUrlFix(): Plugin {
  return {
    name: 'dmxmoney-tauri-legacy-systemjs-url-fix',
    apply: 'build',
    enforce: 'post',
    transformIndexHtml(html) {
      return html.replace(
        "System.import(document.getElementById('vite-legacy-entry').getAttribute('data-src'))",
        [
          '(function(){',
          "var e=document.getElementById('vite-legacy-entry');",
          "var s=e&&e.getAttribute('data-src');",
          'if(!s)return;',
          "if(window.location&&window.location.protocol==='tauri:'&&s.indexOf('://')===-1){",
          "s='tauri://localhost/'+s.replace(/^\\.\\//,'').replace(/^\\//,'');",
          '}',
          'System.import(s);',
          '})()'
        ].join('')
      )
    }
  }
}

// https://vite.dev/config/
export default defineConfig({
  base: './',
  plugins: [
    react(),
    legacy({
      targets: ['safari >= 13', 'ios >= 13', 'chrome >= 71', 'edge >= 79', 'firefox >= 67', 'not IE 11'],
      additionalLegacyPolyfills: ['regenerator-runtime/runtime'],
      polyfills: [
        'es.promise.finally', 
        'es.array.flat-map', 
        'es.array.flat', 
        'es.object.from-entries',
        'es.symbol.description'
      ],
      modernPolyfills: true,
      renderModernChunks: false
    }),
    tauriLegacySystemJsUrlFix()
  ],
  build: {
    target: 'es2015',
    minify: 'terser',
    terserOptions: {
      safari10: true,
    },
    cssTarget: 'safari13'
  }
})
