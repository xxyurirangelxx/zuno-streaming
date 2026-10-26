import { useCallback, useEffect, useState } from 'react';

export type LibraryKind = 'tracks' | 'albums' | 'artists';

export type Route = { name: 'home' } | { name: 'library'; kind: LibraryKind } | { name: 'search' };

export const HOME_ROUTE: Route = { name: 'home' };

export function parseRoute(hash: string): Route {
  const path = hash.replace(/^#\/?/, '').replace(/\/+$/, '');

  if (path === 'search') {
    return { name: 'search' };
  }

  if (path === 'library') {
    return { name: 'library', kind: 'tracks' };
  }

  if (path.startsWith('library/')) {
    const kind = path.slice('library/'.length);

    if (kind === 'albums' || kind === 'artists' || kind === 'tracks') {
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
    case 'library':
      return `#/library/${route.kind}`;
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
