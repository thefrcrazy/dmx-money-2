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

  let isRefreshing = false;
  navigator.serviceWorker.addEventListener('controllerchange', () => {
    if (!isRefreshing) {
      isRefreshing = true;
      window.location.reload();
    }
  });

  const register = async () => {
    try {
      const registration = await navigator.serviceWorker.register('/sw.js');
      void registration.update();
      // Revérifier lors du retour sur l'app
      document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'visible') {
          void registration.update();
        }
      });
    } catch {
      // Échec silencieux
    }
  };

  if (document.readyState === 'complete' || document.readyState === 'interactive') {
    void register();
  } else {
    window.addEventListener('load', () => void register());
  }
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
