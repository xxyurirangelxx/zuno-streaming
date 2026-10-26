import { ZunoApiError } from '@zuno/api-client';
import type { PlaylistSummary, TrackSummary } from '@zuno/shared-types';
import { useCallback, useEffect, useState } from 'react';
import { useSession } from '../app/AuthProvider';
import { describeApiError } from '../lib/errors';
import { Dialog } from './Dialog';
import { PlusIcon } from './Icons';

type LoadState = 'loading' | 'ready' | 'error';

interface Feedback {
  kind: 'success' | 'error';
  message: string;
}

const MAX_NAME_LENGTH = 80;

function isDuplicate(error: unknown): boolean {
  return error instanceof ZunoApiError && error.status === 409;
}

function messageFor(error: unknown, duplicateMessage: string): string {
  return isDuplicate(error)
    ? duplicateMessage
    : describeApiError(
        error instanceof ZunoApiError
          ? error
          : new ZunoApiError('unknown', 'Falha inesperada.', null),
      );
}

export function AddToPlaylistDialog({
  track,
  onClose,
}: {
  track: TrackSummary;
  onClose: () => void;
}) {
  const { client } = useSession();
  const [playlists, setPlaylists] = useState<PlaylistSummary[]>([]);
  const [state, setState] = useState<LoadState>('loading');
  const [name, setName] = useState('');
  const [busyId, setBusyId] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [reloadToken, setReloadToken] = useState(0);

  useEffect(() => {
    const controller = new AbortController();

    client
      .listPlaylists(controller.signal)
      .then((items) => {
        if (controller.signal.aborted) {
          return;
        }

        setPlaylists(items);
        setState('ready');
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) {
          return;
        }

        setState('error');
        setFeedback({ kind: 'error', message: messageFor(cause, 'Playlist já existente.') });
      });

    return () => {
      controller.abort();
    };
  }, [client, reloadToken]);

  const addTo = useCallback(
    async (playlist: PlaylistSummary) => {
      setBusyId(playlist.id);
      setFeedback(null);

      try {
        await client.addPlaylistItem(playlist.id, { track_id: track.id });
        setPlaylists((current) =>
          current.map((item) =>
            item.id === playlist.id ? { ...item, track_count: item.track_count + 1 } : item,
          ),
        );
        setFeedback({
          kind: 'success',
          message: `“${track.title}” foi adicionada a “${playlist.name}”.`,
        });
      } catch (cause) {
        setFeedback({
          kind: 'error',
          message: messageFor(cause, 'Esta faixa já está na playlist.'),
        });
      } finally {
        setBusyId(null);
      }
    },
    [client, track.id, track.title],
  );

  const createAndAdd = useCallback(async () => {
    const trimmed = name.trim();

    if (trimmed.length === 0 || trimmed.length > MAX_NAME_LENGTH) {
      setFeedback({
        kind: 'error',
        message: `Informe um nome entre 1 e ${MAX_NAME_LENGTH} caracteres.`,
      });
      return;
    }

    setCreating(true);
    setFeedback(null);

    try {
      const playlist = await client.createPlaylist({ name: trimmed });

      try {
        await client.addPlaylistItem(playlist.id, { track_id: track.id });
        setPlaylists((current) => [
          { ...playlist, track_count: playlist.track_count + 1 },
          ...current,
        ]);
      } catch (cause) {
        setPlaylists((current) => [playlist, ...current]);

        if (!isDuplicate(cause)) {
          throw cause;
        }
      }

      setName('');
      setFeedback({
        kind: 'success',
        message: `“${track.title}” foi adicionada a “${playlist.name}”.`,
      });
    } catch (cause) {
      setFeedback({
        kind: 'error',
        message: messageFor(cause, 'Já existe uma playlist com esse nome.'),
      });
    } finally {
      setCreating(false);
    }
  }, [client, name, track.id, track.title]);

  return (
    <Dialog
      title="Adicionar à playlist"
      description={`Escolha uma playlist para “${track.title}”.`}
      onClose={onClose}
      footer={
        <button
          type="button"
          onClick={onClose}
          className="inline-flex min-h-10 items-center rounded-xl border border-line px-4 py-2 text-sm font-semibold text-copy hover:bg-surface-muted"
        >
          Fechar
        </button>
      }
    >
      <div role="status" aria-live="polite" className="mb-3 min-h-5 text-sm">
        {feedback && (
          <p className={feedback.kind === 'error' ? 'text-red-500' : 'text-accent-cyan'}>
            {feedback.message}
          </p>
        )}
      </div>

      {state === 'loading' && <p className="text-sm text-copy-muted">Carregando playlists…</p>}

      {state === 'error' && (
        <button
          type="button"
          onClick={() => {
            setState('loading');
            setReloadToken((token) => token + 1);
          }}
          className="inline-flex min-h-10 items-center rounded-xl border border-line px-4 py-2 text-sm font-semibold text-copy hover:bg-surface-muted"
        >
          Tentar novamente
        </button>
      )}

      {state === 'ready' &&
        (playlists.length === 0 ? (
          <p className="rounded-xl border border-dashed border-line/80 p-4 text-sm text-copy-muted">
            Nenhuma playlist ainda. Crie a primeira abaixo.
          </p>
        ) : (
          <ul className="max-h-56 space-y-1 overflow-y-auto">
            {playlists.map((playlist) => (
              <li key={playlist.id}>
                <button
                  type="button"
                  disabled={busyId !== null || creating}
                  onClick={() => {
                    void addTo(playlist);
                  }}
                  className="flex min-h-11 w-full items-center justify-between gap-3 rounded-lg px-3 text-left text-sm text-copy hover:bg-surface-muted disabled:opacity-60"
                >
                  <span className="min-w-0 truncate">{playlist.name}</span>
                  <span className="shrink-0 text-xs text-copy-muted">
                    {busyId === playlist.id ? 'Adicionando…' : `${playlist.track_count} faixas`}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        ))}

      <form
        className="mt-4 border-t border-line/60 pt-4"
        onSubmit={(event) => {
          event.preventDefault();
          void createAndAdd();
        }}
      >
        <label htmlFor="new-playlist-name" className="text-sm font-semibold text-copy">
          Criar nova playlist
        </label>
        <div className="mt-2 flex gap-2">
          <input
            id="new-playlist-name"
            value={name}
            maxLength={MAX_NAME_LENGTH}
            onChange={(event) => {
              setName(event.target.value);
            }}
            placeholder="Nome da playlist"
            className="min-h-10 w-full rounded-xl border border-line bg-surface px-3 text-sm text-copy placeholder:text-copy-muted"
          />
          <button
            type="submit"
            disabled={creating}
            className="primary-button inline-flex min-h-10 shrink-0 items-center gap-1.5 rounded-xl px-3 py-2 text-sm font-semibold disabled:opacity-60"
          >
            <PlusIcon className="h-4 w-4" />
            {creating ? 'Criando…' : 'Criar'}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
