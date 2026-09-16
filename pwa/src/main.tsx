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
  let wasControlled = Boolean(navigator.serviceWorker.controller);
  navigator.serviceWorker.addEventListener('controllerchange', () => {
    if (!wasControlled) {
      wasControlled = true;
      return;
    }
    if (!isRefreshing) {
      isRefreshing = true;
      window.location.reload();
    }
  });

  const register = async () => {
    try {
      const registration = await navigator.serviceWorker.register('/sw.js', { updateViaCache: 'none' });
      let checking = false;
      const checkForUpdate = async () => {
        if (checking || document.visibilityState !== 'visible' || !navigator.onLine) return;
        checking = true;
        try {
          await registration.update();
        } catch {
          // Offline or temporarily unavailable: retry on the next wake or timer.
        } finally {
          checking = false;
        }
      };
      void checkForUpdate();
      document.addEventListener('visibilitychange', () => void checkForUpdate());
      window.addEventListener('focus', () => void checkForUpdate());
      window.addEventListener('pageshow', () => void checkForUpdate());
      window.addEventListener('online', () => void checkForUpdate());
      window.setInterval(() => void checkForUpdate(), 60_000);
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
