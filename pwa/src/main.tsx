import './polyfills';
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'
import { hasTauriRuntime, initializeMobileCompanionToken } from './utils/runtime.ts';

// 1. Les Polyfills de base (ORDRE IMPORTANT)
import '@formatjs/intl-getcanonicallocales/polyfill';
import '@formatjs/intl-locale/polyfill';

// 2. Celui qu'on a ajouté juste avant (NumberFormat)
import '@formatjs/intl-numberformat/polyfill';
import '@formatjs/intl-numberformat/locale-data/fr';

const registerPwaServiceWorker = () => {
  if (hasTauriRuntime() || !('serviceWorker' in navigator)) return;

  const hostname = window.location.hostname;
  const isLocalhost = hostname === 'localhost' || hostname === '127.0.0.1' || hostname === '::1';
  if (!window.isSecureContext && !isLocalhost) return;

  window.addEventListener('load', () => {
    navigator.serviceWorker.register('/sw.js').catch(() => { });
  });
};

const render = () => {
  initializeMobileCompanionToken();
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
};

// Aperçu de l'interface avec des données fictives (`/mobile/?apercu`), en développement seulement.
if (import.meta.env.DEV && new URLSearchParams(window.location.search).has('apercu')) {
  void import('./dev/apercu').then(({ installPreview }) => installPreview()).finally(render);
} else {
  registerPwaServiceWorker();
  render();
}
