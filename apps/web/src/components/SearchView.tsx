import { ZunoApiError } from '@zuno/api-client';
import type { SearchResponse } from '@zuno/shared-types';
import { useEffect, useState } from 'react';
import { useSession } from '../app/AuthProvider';
import { useDebouncedValue } from '../lib/debounce';
import { describeApiError } from '../lib/errors';
import { AlbumGrid, ArtistList } from './MediaCollections';
import { EmptyState, ErrorState, LoadingSkeleton } from './States';
import { CloseIcon, SearchIcon } from './Icons';
import { TrackList } from './TrackList';

type SearchState =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'ready'; data: SearchResponse }
  | { kind: 'error'; error: ZunoApiError };

type SettledSearch =
  { query: string; data: SearchResponse } | { query: string; error: ZunoApiError };

export function SearchView() {
  const { client, expireSession } = useSession();
  const [query, setQuery] = useState('');
  const [settled, setSettled] = useState<SettledSearch | null>(null);
  const trimmed = query.trim();
  const debouncedQuery = useDebouncedValue(trimmed, 300);

  useEffect(() => {
    if (debouncedQuery.length === 0) {
      return;
    }

    const controller = new AbortController();

    client
      .search(debouncedQuery, controller.signal)
      .then((data) => {
        if (!controller.signal.aborted) {
          setSettled({ query: debouncedQuery, data });
        }
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) {
          return;
        }

        setSettled({
          query: debouncedQuery,
          error:
            cause instanceof ZunoApiError
              ? cause
              : new ZunoApiError('unknown', 'Ocorreu uma falha inesperada.', null),
        });
      });

    return () => {
      controller.abort();
    };
  }, [client, debouncedQuery]);

  const state: SearchState =
    trimmed.length === 0
      ? { kind: 'idle' }
      : settled !== null && settled.query === debouncedQuery && debouncedQuery === trimmed
        ? 'data' in settled
          ? { kind: 'ready', data: settled.data }
          : { kind: 'error', error: settled.error }
        : { kind: 'loading' };

  const isUnauthorized = state.kind === 'error' && state.error.status === 401;

  useEffect(() => {
    if (isUnauthorized) {
      expireSession();
    }
  }, [isUnauthorized, expireSession]);

  const isSearching = state.kind === 'loading';
  const total =
    state.kind === 'ready'
      ? state.data.tracks.length + state.data.albums.length + state.data.artists.length
      : 0;

  return (
    <section aria-labelledby="search-heading">
      <p className="text-xs font-semibold tracking-[0.18em] text-accent-cyan uppercase">Buscar</p>
      <h2 id="search-heading" className="mt-1 text-2xl font-semibold text-copy">
        Encontre sua música
      </h2>

      <form role="search" className="mt-5" onSubmit={(event) => event.preventDefault()}>
        <label htmlFor="search-input" className="sr-only">
          Buscar faixas, álbuns e artistas
        </label>
        <div className="glass-panel flex items-center gap-2 rounded-xl px-3 py-1.5">
          <SearchIcon className="h-5 w-5 shrink-0 text-copy-muted" />
          <input
            id="search-input"
            type="search"
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
            }}
            placeholder="Faixa, artista ou álbum…"
            autoComplete="off"
            enterKeyHint="search"
            className="min-h-10 w-full bg-transparent text-sm text-copy outline-none placeholder:text-copy-muted"
          />
          {query.length > 0 && (
            <button
              type="button"
              onClick={() => {
                setQuery('');
              }}
              aria-label="Limpar busca"
              className="inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-full text-copy-muted hover:bg-surface-muted hover:text-copy"
            >
              <CloseIcon className="h-4 w-4" />
            </button>
          )}
        </div>
      </form>

      <div role="status" aria-live="polite" className="sr-only">
        {isSearching ? 'Buscando…' : state.kind === 'ready' ? `${total} resultados` : ''}
      </div>

      <div className="mt-6">
        {state.kind === 'idle' && (
          <EmptyState
            title="Comece digitando"
            description="A busca consulta faixas, álbuns e artistas da sua biblioteca. Os resultados aparecem após uma breve pausa na digitação."
          />
        )}

        {state.kind === 'loading' && <LoadingSkeleton rows={4} label="Buscando…" />}

        {state.kind === 'error' && <ErrorState message={describeApiError(state.error)} />}

        {state.kind === 'ready' && total === 0 && (
          <EmptyState
            title="Nenhum resultado"
            description={`Nada encontrado para “${state.data.query}”. Tente outro termo ou verifique a ortografia.`}
          />
        )}

        {state.kind === 'ready' && total > 0 && (
          <div className="space-y-8">
            {state.data.tracks.length > 0 && (
              <section aria-labelledby="search-tracks">
                <h3 id="search-tracks" className="mb-2 text-lg font-semibold text-copy">
                  Faixas{' '}
                  <span className="text-sm font-normal text-copy-muted">
                    ({state.data.tracks.length})
                  </span>
                </h3>
                <TrackList tracks={state.data.tracks} />
              </section>
            )}

            {state.data.albums.length > 0 && (
              <section aria-labelledby="search-albums">
                <h3 id="search-albums" className="mb-3 text-lg font-semibold text-copy">
                  Álbuns{' '}
                  <span className="text-sm font-normal text-copy-muted">
                    ({state.data.albums.length})
                  </span>
                </h3>
                <AlbumGrid albums={state.data.albums} />
              </section>
            )}

            {state.data.artists.length > 0 && (
              <section aria-labelledby="search-artists">
                <h3 id="search-artists" className="mb-3 text-lg font-semibold text-copy">
                  Artistas{' '}
                  <span className="text-sm font-normal text-copy-muted">
                    ({state.data.artists.length})
                  </span>
                </h3>
                <ArtistList artists={state.data.artists} />
              </section>
            )}
          </div>
        )}
      </div>
    </section>
  );
}
