import type { AlbumSummary, ArtistSummary, TrackSummary } from '@zuno/shared-types';
import { useCallback, useEffect } from 'react';
import { useSession } from '../app/AuthProvider';
import { describeApiError } from '../lib/errors';
import type { LibraryKind } from '../lib/router';
import { routeHref } from '../lib/router';
import { usePagedResource } from '../lib/usePagedResource';
import { AlbumGrid, ArtistList } from './MediaCollections';
import { EmptyState, ErrorState, LoadingSkeleton } from './States';
import { TrackList } from './TrackList';

const TABS: { kind: LibraryKind; label: string }[] = [
  { kind: 'tracks', label: 'Faixas' },
  { kind: 'albums', label: 'Álbuns' },
  { kind: 'artists', label: 'Artistas' },
];

function useLibraryPage(kind: LibraryKind) {
  const { client } = useSession();

  const loadPage = useCallback(
    async (cursor: string | undefined, signal: AbortSignal) => {
      switch (kind) {
        case 'tracks':
          return client.listTracks({ limit: 50, cursor }, signal);
        case 'albums':
          return client.listAlbums({ limit: 50, cursor }, signal);
        case 'artists':
          return client.listArtists({ limit: 50, cursor }, signal);
      }
    },
    [client, kind],
  );

  return usePagedResource<TrackSummary | AlbumSummary | ArtistSummary>(loadPage);
}

interface PagerProps {
  hasMore: boolean;
  loading: boolean;
  onLoadMore: () => void;
}

function LoadMore({ hasMore, loading, onLoadMore }: PagerProps) {
  if (!hasMore) {
    return null;
  }

  return (
    <div className="mt-5 flex justify-center">
      <button
        type="button"
        onClick={onLoadMore}
        disabled={loading}
        className="primary-button inline-flex min-h-11 items-center rounded-xl px-5 py-2 text-sm font-semibold disabled:cursor-not-allowed disabled:opacity-60"
      >
        {loading ? 'Carregando…' : 'Carregar mais'}
      </button>
    </div>
  );
}

export function LibraryView({ kind }: { kind: LibraryKind }) {
  const { expireSession } = useSession();
  const resource = useLibraryPage(kind);

  useEffect(() => {
    if (resource.error?.status === 401) {
      expireSession();
    }
  }, [resource.error, expireSession]);

  const heading = kind === 'tracks' ? 'Todas as faixas' : kind === 'albums' ? 'Álbuns' : 'Artistas';

  if (resource.status === 'loading') {
    return (
      <section aria-labelledby="library-heading">
        <h2 id="library-heading" className="text-2xl font-semibold text-copy">
          {heading}
        </h2>
        <div className="mt-5">
          <LoadingSkeleton label={`Carregando ${heading.toLowerCase()}…`} />
        </div>
      </section>
    );
  }

  if (resource.status === 'error' && resource.error) {
    return (
      <section aria-labelledby="library-heading">
        <h2 id="library-heading" className="text-2xl font-semibold text-copy">
          {heading}
        </h2>
        <div className="mt-5">
          <ErrorState message={describeApiError(resource.error)} onRetry={resource.reload} />
        </div>
      </section>
    );
  }

  const isEmpty = resource.items.length === 0;

  return (
    <section aria-labelledby="library-heading">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="text-xs font-semibold tracking-[0.18em] text-accent-cyan uppercase">
            Biblioteca
          </p>
          <h2 id="library-heading" className="mt-1 text-2xl font-semibold text-copy">
            {heading}
          </h2>
        </div>
        <nav aria-label="Seções da biblioteca">
          <ul className="flex gap-1 rounded-full border border-line/70 bg-surface/60 p-1">
            {TABS.map((tab) => (
              <li key={tab.kind}>
                <a
                  href={routeHref({ name: 'library', kind: tab.kind })}
                  aria-current={tab.kind === kind ? 'page' : undefined}
                  className={`inline-flex min-h-9 items-center rounded-full px-3.5 py-1.5 text-sm font-medium transition-colors ${
                    tab.kind === kind
                      ? 'bg-accent-blue/15 text-accent-blue'
                      : 'text-copy-muted hover:text-copy'
                  }`}
                >
                  {tab.label}
                </a>
              </li>
            ))}
          </ul>
        </nav>
      </div>

      {isEmpty ? (
        <div className="mt-5">
          <EmptyState
            title="Biblioteca vazia"
            description="Envie arquivos Opus pelo cliente de upload para começar a ouvir. Faixas, álbuns e artistas aparecem aqui automaticamente após a indexação."
          />
        </div>
      ) : (
        <div className="mt-5">
          {kind === 'tracks' && <TrackList tracks={resource.items as TrackSummary[]} />}
          {kind === 'albums' && <AlbumGrid albums={resource.items as AlbumSummary[]} />}
          {kind === 'artists' && <ArtistList artists={resource.items as ArtistSummary[]} />}
          <LoadMore
            hasMore={resource.nextCursor !== null}
            loading={resource.loadingMore}
            onLoadMore={resource.loadMore}
          />
        </div>
      )}
    </section>
  );
}
