import { useFavorites } from '../favorites/FavoritesProvider';
import { describeApiError } from '../lib/errors';
import type { LibraryKind } from '../lib/router';
import { routeHref } from '../lib/router';
import { AlbumGrid, ArtistList } from './MediaCollections';
import { EmptyState, ErrorState, LoadingSkeleton } from './States';
import { TrackList } from './TrackList';

const TABS: { kind: LibraryKind; label: string }[] = [
  { kind: 'tracks', label: 'Faixas' },
  { kind: 'albums', label: 'Álbuns' },
  { kind: 'artists', label: 'Artistas' },
];

const EMPTY_COPY: Record<LibraryKind, { title: string; description: string }> = {
  tracks: {
    title: 'Nenhuma faixa favorita',
    description:
      'Toque no coração ao lado de uma faixa para guardá-la aqui. Favoritos são privados e ficam vinculados à sua conta.',
  },
  albums: {
    title: 'Nenhum álbum favorito',
    description:
      'Use o coração nos cartões de álbum da biblioteca ou da busca para acompanhar seus discos preferidos.',
  },
  artists: {
    title: 'Nenhum artista favorito',
    description:
      'Marque artistas com o coração para encontrá-los rapidamente nesta aba. Só você vê seus favoritos.',
  },
};

export function FavoritesView({ kind }: { kind: LibraryKind }) {
  const { data, status, error, reload } = useFavorites();
  const items = kind === 'tracks' ? data.tracks : kind === 'albums' ? data.albums : data.artists;
  const heading =
    kind === 'tracks'
      ? 'Faixas favoritas'
      : kind === 'albums'
        ? 'Álbuns favoritos'
        : 'Artistas favoritos';

  return (
    <section aria-labelledby="favorites-heading">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="text-xs font-semibold tracking-[0.18em] text-accent-cyan uppercase">
            Favoritos
          </p>
          <h2 id="favorites-heading" className="mt-1 text-2xl font-semibold text-copy">
            {heading}
          </h2>
        </div>
        <nav aria-label="Seções dos favoritos">
          <ul className="flex gap-1 rounded-full border border-line/70 bg-surface/60 p-1">
            {TABS.map((tab) => (
              <li key={tab.kind}>
                <a
                  href={routeHref({ name: 'favorites', kind: tab.kind })}
                  aria-current={tab.kind === kind ? 'page' : undefined}
                  className={`inline-flex min-h-9 items-center rounded-full px-3.5 py-1.5 text-sm font-medium transition-colors ${
                    tab.kind === kind
                      ? 'bg-accent-magenta/15 text-accent-magenta'
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

      <div className="mt-5">
        {status === 'loading' && <LoadingSkeleton rows={4} label="Carregando favoritos…" />}

        {status === 'error' && error && (
          <ErrorState message={describeApiError(error)} onRetry={reload} />
        )}

        {status === 'ready' &&
          (items.length === 0 ? (
            <EmptyState title={EMPTY_COPY[kind].title} description={EMPTY_COPY[kind].description} />
          ) : (
            <>
              {kind === 'tracks' && <TrackList tracks={data.tracks} />}
              {kind === 'albums' && <AlbumGrid albums={data.albums} />}
              {kind === 'artists' && <ArtistList artists={data.artists} />}
            </>
          ))}
      </div>
    </section>
  );
}
