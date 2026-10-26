import { useCallback, useEffect, useState } from 'react';

export type LibraryKind = 'tracks' | 'albums' | 'artists';

export type Route =
  | { name: 'home' }
  | { name: 'library'; kind: LibraryKind }
  | { name: 'search' }
  | { name: 'upload' }
  | { name: 'playlists' }
  | { name: 'playlist'; id: string }
  | { name: 'favorites'; kind: LibraryKind }
  | { name: 'history' }
  | { name: 'settings' };

export const HOME_ROUTE: Route = { name: 'home' };

function parseKind(value: string): LibraryKind | null {
  return value === 'albums' || value === 'artists' || value === 'tracks' ? value : null;
}

export function parseRoute(hash: string): Route {
  const path = hash.replace(/^#\/?/, '').replace(/\/+$/, '');

  if (path === 'search') {
    return { name: 'search' };
  }

  if (path === 'upload') {
    return { name: 'upload' };
  }

  if (path === 'playlists') {
    return { name: 'playlists' };
  }

  if (path === 'history') {
    return { name: 'history' };
  }

  if (path === 'settings') {
    return { name: 'settings' };
  }

  if (path.startsWith('playlists/')) {
    const id = decodeURIComponent(path.slice('playlists/'.length));

    if (id.length > 0) {
      return { name: 'playlist', id };
    }
  }

  if (path === 'favorites') {
    return { name: 'favorites', kind: 'tracks' };
  }

  if (path.startsWith('favorites/')) {
    const kind = parseKind(path.slice('favorites/'.length));

    if (kind) {
      return { name: 'favorites', kind };
    }
  }

  if (path === 'library') {
    return { name: 'library', kind: 'tracks' };
  }

  if (path.startsWith('library/')) {
    const kind = parseKind(path.slice('library/'.length));

    if (kind) {
      return { name: 'library', kind };
    }
  }

  return HOME_ROUTE;
}

export function routeHref(route: Route): string {
  switch (route.name) {
    case 'home':
      return '#/home';
    case 'search':
      return '#/search';
    case 'upload':
      return '#/upload';
    case 'library':
      return `#/library/${route.kind}`;
    case 'playlists':
      return '#/playlists';
    case 'playlist':
      return `#/playlists/${encodeURIComponent(route.id)}`;
    case 'favorites':
      return `#/favorites/${route.kind}`;
    case 'history':
      return '#/history';
    case 'settings':
      return '#/settings';
  }
}

export function useRoute(): { route: Route; navigate: (route: Route) => void } {
  const [route, setRoute] = useState<Route>(() => parseRoute(window.location.hash));

  useEffect(() => {
    const onHashChange = () => {
      setRoute(parseRoute(window.location.hash));
    };

    window.addEventListener('hashchange', onHashChange);
    return () => {
      window.removeEventListener('hashchange', onHashChange);
    };
  }, []);

  const navigate = useCallback((next: Route) => {
    window.location.hash = routeHref(next);
    setRoute(next);
  }, []);

  return { route, navigate };
}
