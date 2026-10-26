import { createZunoClient } from '@zuno/api-client';
import type { ZunoClient } from '@zuno/api-client';
import { useMemo } from 'react';
import { AuthProvider, useSession } from './app/AuthProvider';
import { AuthScreen } from './app/AuthScreen';
import { AppShell } from './components/AppShell';
import { FavoritesView } from './components/FavoritesView';
import { HomeView } from './components/HomeView';
import { LibraryView } from './components/LibraryView';
import { PlaylistDetailView } from './components/PlaylistDetailView';
import { PlaylistsView } from './components/PlaylistsView';
import { SearchView } from './components/SearchView';
import { UploadView } from './components/UploadView';
import { ErrorState } from './components/States';
import { FavoritesProvider } from './favorites/FavoritesProvider';
import { describeApiError } from './lib/errors';
import { useRoute } from './lib/router';
import { PlayerProvider } from './player/PlayerProvider';

const defaultBaseUrl = import.meta.env.VITE_ZUNO_API_BASE_URL ?? '';

function Splash() {
  return (
    <div
      role="status"
      aria-live="polite"
      className="flex min-h-screen flex-col items-center justify-center gap-4 px-6"
    >
      <img src="/logo.png" alt="" className="h-16 w-auto max-w-44 object-contain" />
      <p className="text-sm text-copy-muted">Verificando sessão…</p>
      <div aria-hidden="true" className="h-1 w-40 overflow-hidden rounded-full bg-surface-muted">
        <div className="h-full w-1/2 rounded-full bg-accent-blue motion-safe:animate-pulse" />
      </div>
    </div>
  );
}

function SessionProblem() {
  const { state, retry } = useSession();

  if (state.kind !== 'error') {
    return null;
  }

  return (
    <div className="mx-auto flex min-h-screen w-full max-w-2xl items-center px-4 py-10">
      <div className="w-full">
        <ErrorState
          title="Não foi possível conectar ao Zuno"
          message={describeApiError(state.error)}
          onRetry={retry}
        />
      </div>
    </div>
  );
}

function Routes() {
  const { route } = useRoute();

  switch (route.name) {
    case 'home':
      return <HomeView />;
    case 'search':
      return <SearchView />;
    case 'upload':
      return <UploadView />;
    case 'library':
      return <LibraryView kind={route.kind} />;
    case 'playlists':
      return <PlaylistsView />;
    case 'playlist':
      return <PlaylistDetailView id={route.id} />;
    case 'favorites':
      return <FavoritesView kind={route.kind} />;
  }
}

function SessionGate() {
  const { client, state } = useSession();

  if (state.kind === 'loading') {
    return <Splash />;
  }

  if (state.kind === 'error') {
    return <SessionProblem />;
  }

  if (state.kind === 'setup' || state.kind === 'login') {
    return <AuthScreen />;
  }

  return (
    <FavoritesProvider client={client}>
      <PlayerProvider client={client}>
        <AppShell>
          <Routes />
        </AppShell>
      </PlayerProvider>
    </FavoritesProvider>
  );
}

export interface AppProps {
  /** Injeção usada nos testes; em produção o cliente é criado aqui. */
  client?: ZunoClient;
}

export default function App({ client }: AppProps) {
  const createdClient = useMemo(() => createZunoClient({ baseUrl: defaultBaseUrl }), []);
  const activeClient = client ?? createdClient;

  return (
    <AuthProvider client={activeClient}>
      <SessionGate />
    </AuthProvider>
  );
}
