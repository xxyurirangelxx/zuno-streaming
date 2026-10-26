import type { ZunoClient } from '@zuno/api-client';
import { ZunoApiError } from '@zuno/api-client';
import type {
  AlbumSummary,
  ArtistSummary,
  FavoriteKind,
  FavoritesResponse,
  TrackSummary,
} from '@zuno/shared-types';
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import type { ReactNode } from 'react';
import { describeApiError } from '../lib/errors';

export type FavoriteEntity = TrackSummary | AlbumSummary | ArtistSummary;

export type FavoritesStatus = 'loading' | 'ready' | 'error';

interface Feedback {
  kind: 'success' | 'error';
  message: string;
}

export interface FavoritesContextValue {
  data: FavoritesResponse;
  status: FavoritesStatus;
  error: ZunoApiError | null;
  reload: () => void;
  isFavorite: (kind: FavoriteKind, id: string) => boolean;
  toggleFavorite: (kind: FavoriteKind, entity: FavoriteEntity) => Promise<void>;
  feedback: Feedback | null;
}

const EMPTY: FavoritesResponse = { tracks: [], albums: [], artists: [] };

const FavoritesContext = createContext<FavoritesContextValue | null>(null);

function toApiError(error: unknown): ZunoApiError {
  return error instanceof ZunoApiError
    ? error
    : new ZunoApiError('unknown', 'Ocorreu uma falha inesperada.', null);
}

function idsOf(data: FavoritesResponse, kind: FavoriteKind): Set<string> {
  switch (kind) {
    case 'track':
      return new Set(data.tracks.map((track) => track.id));
    case 'album':
      return new Set(data.albums.map((album) => album.id));
    case 'artist':
      return new Set(data.artists.map((artist) => artist.id));
  }
}

function addEntity(data: FavoritesResponse, kind: FavoriteKind, entity: FavoriteEntity) {
  if (idsOf(data, kind).has(entity.id)) {
    return data;
  }

  switch (kind) {
    case 'track':
      return { ...data, tracks: [...data.tracks, entity as TrackSummary] };
    case 'album':
      return { ...data, albums: [...data.albums, entity as AlbumSummary] };
    case 'artist':
      return { ...data, artists: [...data.artists, entity as ArtistSummary] };
  }
}

function removeEntity(data: FavoritesResponse, kind: FavoriteKind, id: string) {
  switch (kind) {
    case 'track':
      return { ...data, tracks: data.tracks.filter((track) => track.id !== id) };
    case 'album':
      return { ...data, albums: data.albums.filter((album) => album.id !== id) };
    case 'artist':
      return { ...data, artists: data.artists.filter((artist) => artist.id !== id) };
  }
}

export function FavoritesProvider({
  client,
  children,
}: {
  client: ZunoClient;
  children: ReactNode;
}) {
  const [data, setData] = useState<FavoritesResponse>(EMPTY);
  const [status, setStatus] = useState<FavoritesStatus>('loading');
  const [error, setError] = useState<ZunoApiError | null>(null);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const dataRef = useRef<FavoritesResponse>(EMPTY);

  const commit = useCallback((next: FavoritesResponse) => {
    dataRef.current = next;
    setData(next);
  }, []);

  useEffect(() => {
    const controller = new AbortController();

    client
      .listFavorites(controller.signal)
      .then((response) => {
        if (controller.signal.aborted) {
          return;
        }

        commit(response);
        setStatus('ready');
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) {
          return;
        }

        setError(toApiError(cause));
        setStatus('error');
      });

    return () => {
      controller.abort();
    };
  }, [client, commit, reloadToken]);

  const reload = useCallback(() => {
    setStatus('loading');
    setError(null);
    setReloadToken((token) => token + 1);
  }, []);

  const isFavorite = useCallback(
    (kind: FavoriteKind, id: string) => idsOf(data, kind).has(id),
    [data],
  );

  const toggleFavorite = useCallback(
    async (kind: FavoriteKind, entity: FavoriteEntity) => {
      const previous = dataRef.current;
      const wasFavorite = idsOf(previous, kind).has(entity.id);
      const next = wasFavorite
        ? removeEntity(previous, kind, entity.id)
        : addEntity(previous, kind, entity);

      commit(next);

      try {
        await client.setFavorite(kind, entity.id, !wasFavorite);
        setFeedback({
          kind: 'success',
          message: wasFavorite ? 'Removido dos favoritos.' : 'Adicionado aos favoritos.',
        });
      } catch (cause) {
        commit(previous);
        setFeedback({ kind: 'error', message: describeApiError(toApiError(cause)) });
      }
    },
    [client, commit],
  );

  const value = useMemo<FavoritesContextValue>(
    () => ({
      data,
      status,
      error,
      reload,
      isFavorite,
      toggleFavorite,
      feedback,
    }),
    [data, status, error, reload, isFavorite, toggleFavorite, feedback],
  );

  return (
    <FavoritesContext.Provider value={value}>
      {children}
      <div
        role={feedback?.kind === 'error' ? 'alert' : 'status'}
        aria-live={feedback?.kind === 'error' ? 'assertive' : 'polite'}
        className="sr-only"
      >
        {feedback?.message ?? ''}
      </div>
    </FavoritesContext.Provider>
  );
}

export function useFavorites(): FavoritesContextValue {
  const context = useContext(FavoritesContext);

  if (!context) {
    throw new Error('useFavorites precisa estar dentro de FavoritesProvider.');
  }

  return context;
}

export function useFavoritesOptional(): FavoritesContextValue | null {
  return useContext(FavoritesContext);
}
