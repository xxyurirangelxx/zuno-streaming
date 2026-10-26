import type { ReactNode } from 'react';
import { useSession } from '../app/AuthProvider';
import { formatRole } from '../lib/format';
import type { Route } from '../lib/router';
import { routeHref, useRoute } from '../lib/router';
import { PlayerBar } from '../player/PlayerBar';
import {
  HeartIcon,
  HomeIcon,
  LibraryIcon,
  LogoutIcon,
  PlaylistIcon,
  SearchIcon,
  UploadIcon,
} from './Icons';

interface NavItem {
  route: Route;
  label: string;
}

const NAV_ITEMS: NavItem[] = [
  { route: { name: 'home' }, label: 'Início' },
  { route: { name: 'library', kind: 'tracks' }, label: 'Biblioteca' },
  { route: { name: 'playlists' }, label: 'Playlists' },
  { route: { name: 'favorites', kind: 'tracks' }, label: 'Favoritos' },
  { route: { name: 'search' }, label: 'Buscar' },
  { route: { name: 'upload' }, label: 'Enviar' },
];

function isActive(current: Route, item: Route): boolean {
  if (current.name === 'playlist' && item.name === 'playlists') {
    return true;
  }

  return current.name === item.name;
}

function NavIcon({ route }: { route: Route }) {
  if (route.name === 'home') {
    return <HomeIcon className="h-5 w-5" />;
  }

  if (route.name === 'search') {
    return <SearchIcon className="h-5 w-5" />;
  }

  if (route.name === 'upload') {
    return <UploadIcon className="h-5 w-5" />;
  }

  if (route.name === 'playlists' || route.name === 'playlist') {
    return <PlaylistIcon className="h-5 w-5" />;
  }

  if (route.name === 'favorites') {
    return <HeartIcon className="h-5 w-5" />;
  }

  return <LibraryIcon className="h-5 w-5" />;
}

export function AppShell({ children }: { children: ReactNode }) {
  const { state, logout } = useSession();
  const { route } = useRoute();
  const user = state.kind === 'authenticated' ? state.user : null;

  return (
    <div className="min-h-screen">
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-50 focus:rounded-lg focus:bg-surface focus:px-4 focus:py-2 focus:text-sm focus:font-semibold focus:text-copy"
      >
        Pular para o conteúdo
      </a>

      <aside className="fixed inset-y-0 left-0 z-20 hidden w-60 flex-col border-r border-line/60 bg-surface/70 px-4 pt-6 pb-28 backdrop-blur-xl md:flex">
        <div className="flex items-center gap-3 px-2">
          <img src="/logo.png" alt="" className="h-11 w-auto max-w-28 object-contain" />
          <span className="text-xl font-semibold tracking-tight text-copy">Zuno</span>
        </div>

        <nav aria-label="Navegação principal" className="mt-8 flex-1">
          <ul className="space-y-1">
            {NAV_ITEMS.map((item) => {
              const active = isActive(route, item.route);

              return (
                <li key={item.label}>
                  <a
                    href={routeHref(item.route)}
                    aria-current={active ? 'page' : undefined}
                    className={`flex min-h-11 items-center gap-3 rounded-xl px-3 text-sm font-medium transition-colors ${
                      active
                        ? 'bg-accent-blue/15 text-accent-blue'
                        : 'text-copy-muted hover:bg-surface-muted hover:text-copy'
                    }`}
                  >
                    <NavIcon route={item.route} />
                    {item.label}
                  </a>
                </li>
              );
            })}
          </ul>
        </nav>

        <div className="mt-4 rounded-xl border border-line/60 bg-surface/60 p-3">
          <p className="truncate text-sm font-semibold text-copy">{user?.username ?? '—'}</p>
          <p className="text-xs text-copy-muted">{user ? formatRole(user.role) : ''}</p>
          <button
            type="button"
            onClick={() => {
              void logout();
            }}
            className="mt-3 inline-flex min-h-9 w-full items-center justify-center gap-2 rounded-lg border border-line px-3 py-1.5 text-xs font-semibold text-copy hover:bg-surface-muted"
          >
            <LogoutIcon className="h-4 w-4" />
            Sair
          </button>
        </div>
      </aside>

      <header className="flex items-center justify-between gap-3 border-b border-line/60 px-4 py-3 md:hidden">
        <div className="flex items-center gap-2">
          <img src="/logo.png" alt="" className="h-9 w-auto max-w-24 object-contain" />
          <span className="text-lg font-semibold tracking-tight text-copy">Zuno</span>
        </div>
        <div className="flex items-center gap-2">
          <div className="text-right">
            <p className="max-w-28 truncate text-xs font-semibold text-copy">{user?.username}</p>
            <p className="text-[0.65rem] text-copy-muted">{user ? formatRole(user.role) : ''}</p>
          </div>
          <button
            type="button"
            onClick={() => {
              void logout();
            }}
            aria-label="Sair"
            className="inline-flex h-9 w-9 items-center justify-center rounded-full border border-line text-copy-muted hover:text-copy"
          >
            <LogoutIcon className="h-4 w-4" />
          </button>
        </div>
      </header>

      <main id="main-content" className="pb-56 md:ml-60 md:pb-28">
        <div className="mx-auto w-full max-w-6xl px-4 py-6 sm:px-8 sm:py-10">{children}</div>
      </main>

      <PlayerBar />

      <nav
        aria-label="Navegação principal"
        className="fixed inset-x-0 bottom-0 z-40 border-t border-line/60 bg-surface/90 backdrop-blur-xl md:hidden"
      >
        <ul className="flex items-stretch justify-around">
          {NAV_ITEMS.map((item) => {
            const active = isActive(route, item.route);

            return (
              <li key={item.label} className="flex-1">
                <a
                  href={routeHref(item.route)}
                  aria-current={active ? 'page' : undefined}
                  className={`flex min-h-16 flex-col items-center justify-center gap-1 text-[0.7rem] font-medium ${
                    active ? 'text-accent-blue' : 'text-copy-muted'
                  }`}
                >
                  <NavIcon route={item.route} />
                  {item.label}
                </a>
              </li>
            );
          })}
        </ul>
      </nav>
    </div>
  );
}
