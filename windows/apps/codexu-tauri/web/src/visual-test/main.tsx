import React from 'react';
import ReactDOM from 'react-dom/client';
import { Dashboard } from '../windows/Dashboard';
import { Settings } from '../windows/Settings';
import { I18nProvider } from '../i18n/I18nProvider';
import '../index.css';

const SURFACE_TITLES = {
  overview: 'Overview',
  tasks: 'Tasks',
  leadership: 'AI Leadership',
  usage: 'Usage',
  inference: 'Inference',
  projects: 'Projects',
  skills: 'Skills',
  settings: 'Settings',
} as const;

type SurfaceId = keyof typeof SURFACE_TITLES;

function VisualTestApp() {
  const requestedSurface = new URLSearchParams(window.location.search).get('surface');
  const surface = isSurfaceId(requestedSurface) ? requestedSurface : null;

  if (!surface) {
    return <main data-testid="visual-test-unsupported">Unsupported visual test surface</main>;
  }

  return (
    <main
      className="windows-glass-page min-h-screen"
      data-testid="visual-test-surface"
      data-surface={surface}
    >
      <h2 className="sr-only">{SURFACE_TITLES[surface]}</h2>
      {surface === 'settings' ? <Settings /> : <Dashboard />}
    </main>
  );
}

function isSurfaceId(value: string | null): value is SurfaceId {
  return value !== null && value in SURFACE_TITLES;
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <I18nProvider>
      <VisualTestApp />
    </I18nProvider>
  </React.StrictMode>,
);
