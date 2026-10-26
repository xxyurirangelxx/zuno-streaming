import { ZunoApiError } from '@zuno/api-client';
import type { PlaylistDetail, TrackSummary } from '@zuno/shared-types';
import { useCallback, useEffect, useState } from 'react';
import { useSession } from '../app/AuthProvider';
import { describeApiError } from '../lib/errors';
import { routeHref } from '../lib/router';
import { usePlayer } from '../player/PlayerProvider';
import { ChevronDownIcon, ChevronUpIcon, PlayIcon, TrashIcon } from './Icons';
import { EmptyState, ErrorState, LoadingSkeleton } from './States';
import { TrackList } from './TrackList';

type LoadStatus = 'loading' | 'ready' | 'error';

function toApiError(error: unknown): ZunoApiError {
  return error instanceof ZunoApiError
    ? error
    : new ZunoApiError('unknown', 'Ocorreu uma falha inesperada.', null);
}

export function PlaylistDetailView({ id }: { id: string }) {
  const { client } = useSession();
  const { dispatch } = usePlayer();
  const [playlist, setPlaylist] = useState<PlaylistDetail | null>(null);
  const [status, setStatus] = useState<LoadStatus>('loading');
  const [error, setError] = useState<ZunoApiError | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const [busyTrackId, setBusyTrackId] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();

    client
      .getPlaylist(id, controller.signal)
      .then((response) => {
        if (controller.signal.aborted) {
          return;
        }

        setPlaylist(response);
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
  }, [client, id, reloadToken]);

  const moveTrack = useCallback(
    async (track: TrackSummary, index: number, offset: -1 | 1) => {
      if (!playlist) {
        return;
      }

      const target = index + offset;
      const items = playlist.items;

      if (target < 0 || target >= items.length) {
        return;
      }

      const previous = playlist;
      const reordered = [...items];
      const [moved] = reordered.splice(index, 1);

      if (!moved) {
        return;
      }

      reordered.splice(target, 0, moved);
      setPlaylist({ ...playlist, items: reordered });
      setBusyTrackId(track.id);
      setFeedback(null);

      try {
        // Posições da API são baseadas em zero, na ordem exibida.
        await client.movePlaylistItem(playlist.id, track.id, { position: target });
        setFeedback(`“${track.title}” movida para a posição ${target + 1}.`);
      } catch (cause) {
        setPlaylist(previous);
        setFeedback(describeApiError(toApiError(cause)));
      } finally {
        setBusyTrackId(null);
      }
    },
    [client, playlist],
  );

  const removeTrack = useCallback(
    async (track: TrackSummary) => {
      if (!playlist) {
        return;
      }

      setBusyTrackId(track.id);
      setFeedback(null);

      try {
        await client.removePlaylistItem(playlist.id, track.id);
        setPlaylist({
          ...playlist,
          items: playlist.items.filter((item) => item.id !== track.id),
          track_count: Math.max(0, playlist.track_count - 1),
        });
        setFeedback(`“${track.title}” removida da playlist.`);
      } catch (cause) {
        setFeedback(describeApiError(toApiError(cause)));
      } finally {
        setBusyTrackId(null);
      }
    },
    [client, playlist],
  );

  const playAll = useCallback(() => {
    if (!playlist || playlist.items.length === 0) {
      return;
    }

    dispatch({ type: 'playQueue', tracks: playlist.items, startIndex: 0 });
  }, [dispatch, playlist]);

  if (status === 'loading') {
    return (
      <section aria-labelledby="playlist-heading">
        <h2 id="playlist-heading" className="text-2xl font-semibold text-copy">
          Playlist
        </h2>
        <div className="mt-5">
          <LoadingSkeleton label="Carregando playlist…" />
        </div>
      </section>
    );
  }

  if (status === 'error' || !playlist) {
    return (
      <section aria-labelledby="playlist-heading">
        <h2 id="playlist-heading" className="text-2xl font-semibold text-copy">
          Playlist
        </h2>
        <div className="mt-5">
          <ErrorState
            message={error ? describeApiError(error) : 'Playlist não encontrada.'}
            onRetry={() => {
              setStatus('loading');
              setError(null);
              setReloadToken((token) => token + 1);
            }}
          />
        </div>
        <p className="mt-4 text-sm">
          <a href={routeHref({ name: 'playlists' })} className="text-accent-blue hover:underline">
            Voltar para as playlists
          </a>
        </p>
      </section>
    );
  }

  return (
    <section aria-labelledby="playlist-heading">
      <p className="text-xs font-semibold tracking-[0.18em] text-accent-cyan uppercase">
        <a href={routeHref({ name: 'playlists' })} className="hover:underline">
          Playlists
        </a>{' '}
        / detalhe
      </p>
      <div className="mt-1 flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 id="playlist-heading" className="text-2xl font-semibold text-copy">
            {playlist.name}
          </h2>
          <p className="mt-1 text-sm text-copy-muted">
            {playlist.track_count} {playlist.track_count === 1 ? 'faixa' : 'faixas'} ·{' '}
            {playlist.visibility === 'private' ? 'Privada' : 'Não listada'}
          </p>
        </div>
        <button
          type="button"
          onClick={playAll}
          disabled={playlist.items.length === 0}
          className="primary-button inline-flex min-h-11 items-center gap-2 rounded-xl px-4 py-2 text-sm font-semibold disabled:cursor-not-allowed disabled:opacity-60"
        >
          <PlayIcon className="h-4 w-4" />
          Tocar tudo
        </button>
      </div>

      <div role="status" aria-live="polite" className="mt-3 min-h-5 text-sm text-copy-muted">
        {feedback ?? ''}
      </div>

      {playlist.items.length === 0 ? (
        <EmptyState
          title="Playlist vazia"
          description="Playlist vazia — adicione faixas pelo botão de playlist em qualquer música da biblioteca."
        />
      ) : (
        <div className="mt-2">
          <TrackList
            tracks={playlist.items}
            showAlbum
            actions={(track, index) => (
              <div className="flex shrink-0 items-center gap-1">
                <button
                  type="button"
                  disabled={index === 0 || busyTrackId !== null}
                  onClick={() => {
                    void moveTrack(track, index, -1);
                  }}
                  aria-label={`Mover ${track.title} para cima`}
                  title="Mover para cima"
                  className="inline-flex h-8 w-8 items-center justify-center rounded-full text-copy-muted hover:bg-surface-muted hover:text-copy disabled:opacity-30"
                >
                  <ChevronUpIcon className="h-4 w-4" />
                </button>
                <button
                  type="button"
                  disabled={index === playlist.items.length - 1 || busyTrackId !== null}
                  onClick={() => {
                    void moveTrack(track, index, 1);
                  }}
                  aria-label={`Mover ${track.title} para baixo`}
                  title="Mover para baixo"
                  className="inline-flex h-8 w-8 items-center justify-center rounded-full text-copy-muted hover:bg-surface-muted hover:text-copy disabled:opacity-30"
                >
                  <ChevronDownIcon className="h-4 w-4" />
                </button>
                <button
                  type="button"
                  disabled={busyTrackId !== null}
                  onClick={() => {
                    void removeTrack(track);
                  }}
                  aria-label={`Remover ${track.title} da playlist`}
                  title="Remover da playlist"
                  className="inline-flex h-8 w-8 items-center justify-center rounded-full text-copy-muted hover:bg-red-500/10 hover:text-red-500 disabled:opacity-30"
                >
                  <TrashIcon className="h-4 w-4" />
                </button>
              </div>
            )}
          />
        </div>
      )}
    </section>
  );
}
