import { ZunoApiError } from '@zuno/api-client';
import type { PlaylistSummary } from '@zuno/shared-types';
import { useCallback, useEffect, useState } from 'react';
import { useSession } from '../app/AuthProvider';
import { describeApiError } from '../lib/errors';
import { routeHref } from '../lib/router';
import { Dialog } from './Dialog';
import { EditIcon, PlaylistIcon, PlusIcon, TrashIcon } from './Icons';
import { EmptyState, ErrorState, LoadingSkeleton } from './States';

const MAX_NAME_LENGTH = 80;

type LoadStatus = 'loading' | 'ready' | 'error';

type DialogState =
  | { kind: 'create' }
  | { kind: 'rename'; playlist: PlaylistSummary }
  | { kind: 'delete'; playlist: PlaylistSummary }
  | null;

interface Feedback {
  kind: 'success' | 'error';
  message: string;
}

function toApiError(error: unknown): ZunoApiError {
  return error instanceof ZunoApiError
    ? error
    : new ZunoApiError('unknown', 'Ocorreu uma falha inesperada.', null);
}

function validName(name: string): string | null {
  const trimmed = name.trim();

  if (trimmed.length === 0) {
    return 'Informe um nome para a playlist.';
  }

  if (trimmed.length > MAX_NAME_LENGTH) {
    return `O nome deve ter no máximo ${MAX_NAME_LENGTH} caracteres.`;
  }

  return null;
}

export function PlaylistsView() {
  const { client } = useSession();
  const [playlists, setPlaylists] = useState<PlaylistSummary[]>([]);
  const [status, setStatus] = useState<LoadStatus>('loading');
  const [loadError, setLoadError] = useState<ZunoApiError | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const [dialog, setDialog] = useState<DialogState>(null);
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<Feedback | null>(null);

  useEffect(() => {
    const controller = new AbortController();

    client
      .listPlaylists(controller.signal)
      .then((items) => {
        if (controller.signal.aborted) {
          return;
        }

        setPlaylists(items);
        setStatus('ready');
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) {
          return;
        }

        setLoadError(toApiError(cause));
        setStatus('error');
      });

    return () => {
      controller.abort();
    };
  }, [client, reloadToken]);

  const openCreate = useCallback(() => {
    setName('');
    setFormError(null);
    setDialog({ kind: 'create' });
  }, []);

  const openRename = useCallback((playlist: PlaylistSummary) => {
    setName(playlist.name);
    setFormError(null);
    setDialog({ kind: 'rename', playlist });
  }, []);

  const submitName = useCallback(async () => {
    const invalid = validName(name);

    if (invalid) {
      setFormError(invalid);
      return;
    }

    if (!dialog || dialog.kind === 'delete') {
      return;
    }

    setBusy(true);
    setFormError(null);

    try {
      if (dialog.kind === 'create') {
        const created = await client.createPlaylist({ name: name.trim() });
        setPlaylists((current) => [created, ...current]);
        setFeedback({ kind: 'success', message: `Playlist “${created.name}” criada.` });
      } else {
        const updated = await client.updatePlaylist(dialog.playlist.id, { name: name.trim() });
        setPlaylists((current) => current.map((item) => (item.id === updated.id ? updated : item)));
        setFeedback({ kind: 'success', message: `Playlist renomeada para “${updated.name}”.` });
      }

      setDialog(null);
    } catch (cause) {
      const error = toApiError(cause);
      setFormError(
        error.status === 409 ? 'Já existe uma playlist com esse nome.' : describeApiError(error),
      );
    } finally {
      setBusy(false);
    }
  }, [client, dialog, name]);

  const confirmDelete = useCallback(async () => {
    if (!dialog || dialog.kind !== 'delete') {
      return;
    }

    setBusy(true);

    try {
      await client.deletePlaylist(dialog.playlist.id);
      setPlaylists((current) => current.filter((item) => item.id !== dialog.playlist.id));
      setFeedback({ kind: 'success', message: `Playlist “${dialog.playlist.name}” excluída.` });
      setDialog(null);
    } catch (cause) {
      setFeedback({ kind: 'error', message: describeApiError(toApiError(cause)) });
    } finally {
      setBusy(false);
    }
  }, [client, dialog]);

  return (
    <section aria-labelledby="playlists-heading">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="text-xs font-semibold tracking-[0.18em] text-accent-cyan uppercase">
            Playlists
          </p>
          <h2 id="playlists-heading" className="mt-1 text-2xl font-semibold text-copy">
            Suas playlists
          </h2>
        </div>
        <button
          type="button"
          onClick={openCreate}
          className="primary-button inline-flex min-h-11 items-center gap-2 rounded-xl px-4 py-2 text-sm font-semibold"
        >
          <PlusIcon className="h-4 w-4" />
          Nova playlist
        </button>
      </div>

      <div role="status" aria-live="polite" className="mt-3 min-h-5 text-sm">
        {feedback && (
          <p className={feedback.kind === 'error' ? 'text-red-500' : 'text-accent-cyan'}>
            {feedback.message}
          </p>
        )}
      </div>

      {status === 'loading' && <LoadingSkeleton rows={3} label="Carregando playlists…" />}

      {status === 'error' && loadError && (
        <ErrorState
          message={describeApiError(loadError)}
          onRetry={() => {
            setStatus('loading');
            setLoadError(null);
            setReloadToken((token) => token + 1);
          }}
        />
      )}

      {status === 'ready' &&
        (playlists.length === 0 ? (
          <EmptyState
            title="Nenhuma playlist ainda"
            description="Crie uma playlist e adicione faixas pelo botão de playlist em qualquer música."
          >
            <button
              type="button"
              onClick={openCreate}
              className="primary-button inline-flex min-h-11 items-center gap-2 rounded-xl px-4 py-2 text-sm font-semibold"
            >
              <PlusIcon className="h-4 w-4" />
              Criar playlist
            </button>
          </EmptyState>
        ) : (
          <ul className="mt-2 divide-y divide-line/50">
            {playlists.map((playlist) => (
              <li
                key={playlist.id}
                className="flex flex-wrap items-center gap-3 rounded-xl px-2 py-3 hover:bg-surface-muted/60"
              >
                <span
                  aria-hidden="true"
                  className="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg bg-accent-indigo/15 text-accent-indigo"
                >
                  <PlaylistIcon className="h-5 w-5" />
                </span>

                <div className="min-w-0 flex-1">
                  <a
                    href={routeHref({ name: 'playlist', id: playlist.id })}
                    className="block truncate text-sm font-semibold text-copy hover:text-accent-blue"
                  >
                    {playlist.name}
                  </a>
                  <p className="text-xs text-copy-muted">
                    {playlist.track_count} {playlist.track_count === 1 ? 'faixa' : 'faixas'} ·{' '}
                    {playlist.visibility === 'private' ? 'Privada' : 'Não listada'}
                  </p>
                </div>

                <div className="flex items-center gap-1">
                  <button
                    type="button"
                    onClick={() => {
                      openRename(playlist);
                    }}
                    aria-label={`Renomear ${playlist.name}`}
                    title="Renomear"
                    className="inline-flex h-9 w-9 items-center justify-center rounded-full text-copy-muted hover:bg-surface-muted hover:text-copy"
                  >
                    <EditIcon className="h-4 w-4" />
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setDialog({ kind: 'delete', playlist });
                    }}
                    aria-label={`Excluir ${playlist.name}`}
                    title="Excluir"
                    className="inline-flex h-9 w-9 items-center justify-center rounded-full text-copy-muted hover:bg-red-500/10 hover:text-red-500"
                  >
                    <TrashIcon className="h-4 w-4" />
                  </button>
                </div>
              </li>
            ))}
          </ul>
        ))}

      {(dialog?.kind === 'create' || dialog?.kind === 'rename') && (
        <Dialog
          title={dialog.kind === 'create' ? 'Nova playlist' : 'Renomear playlist'}
          onClose={() => {
            setDialog(null);
          }}
          footer={
            <>
              <button
                type="button"
                onClick={() => {
                  setDialog(null);
                }}
                className="inline-flex min-h-10 items-center rounded-xl border border-line px-4 py-2 text-sm font-semibold text-copy hover:bg-surface-muted"
              >
                Cancelar
              </button>
              <button
                type="submit"
                form="playlist-name-form"
                disabled={busy}
                className="primary-button inline-flex min-h-10 items-center rounded-xl px-4 py-2 text-sm font-semibold disabled:opacity-60"
              >
                {busy ? 'Salvando…' : dialog.kind === 'create' ? 'Criar' : 'Salvar'}
              </button>
            </>
          }
        >
          <form
            id="playlist-name-form"
            onSubmit={(event) => {
              event.preventDefault();
              void submitName();
            }}
          >
            <label htmlFor="playlist-name" className="text-sm font-semibold text-copy">
              Nome da playlist
            </label>
            <input
              id="playlist-name"
              value={name}
              maxLength={MAX_NAME_LENGTH}
              data-autofocus
              onChange={(event) => {
                setName(event.target.value);
                setFormError(null);
              }}
              aria-invalid={formError !== null}
              aria-describedby={formError ? 'playlist-name-error' : undefined}
              className="mt-2 min-h-11 w-full rounded-xl border border-line bg-surface px-3 text-sm text-copy"
            />
            {formError && (
              <p id="playlist-name-error" role="alert" className="mt-2 text-sm text-red-500">
                {formError}
              </p>
            )}
          </form>
        </Dialog>
      )}

      {dialog?.kind === 'delete' && (
        <Dialog
          title="Excluir playlist"
          description={`“${dialog.playlist.name}” será removida permanentemente. As faixas continuam na biblioteca.`}
          onClose={() => {
            setDialog(null);
          }}
          footer={
            <>
              <button
                type="button"
                data-autofocus
                onClick={() => {
                  setDialog(null);
                }}
                className="inline-flex min-h-10 items-center rounded-xl border border-line px-4 py-2 text-sm font-semibold text-copy hover:bg-surface-muted"
              >
                Cancelar
              </button>
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  void confirmDelete();
                }}
                className="inline-flex min-h-10 items-center gap-2 rounded-xl border border-red-500/60 bg-red-500/10 px-4 py-2 text-sm font-semibold text-red-500 disabled:opacity-60"
              >
                <TrashIcon className="h-4 w-4" />
                {busy ? 'Excluindo…' : 'Excluir playlist'}
              </button>
            </>
          }
        >
          <p className="text-sm text-copy-muted">Esta ação não pode ser desfeita.</p>
        </Dialog>
      )}
    </section>
  );
}

export { validName, MAX_NAME_LENGTH };
