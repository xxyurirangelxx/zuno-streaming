import { ZunoApiError } from '@zuno/api-client';
import type { HistoryItem } from '@zuno/shared-types';
import { useCallback, useEffect, useState } from 'react';
import { useSession } from '../app/AuthProvider';
import { describeApiError } from '../lib/errors';
import { formatClock, formatDateTime, formatHistoryEvent } from '../lib/format';
import { routeHref } from '../lib/router';
import { usePagedResource } from '../lib/usePagedResource';
import { Artwork } from './Artwork';
import { Dialog } from './Dialog';
import { EmptyState, ErrorState, LoadingSkeleton } from './States';
import { TrashIcon } from './Icons';

const PAGE_SIZE = 30;

function toApiError(error: unknown): ZunoApiError {
  if (error instanceof ZunoApiError) {
    return error;
  }

  return new ZunoApiError('unknown', 'Ocorreu uma falha inesperada.', null);
}

function HistoryRow({ item }: { item: HistoryItem }) {
  const artists =
    item.track.artists.map((artist) => artist.name).join(', ') || 'Artista desconhecido';
  const showSeconds = item.seconds !== null && item.seconds > 0 && item.event_kind !== 'skip';

  return (
    <li className="flex items-center gap-3 rounded-xl px-2 py-2.5 transition-colors hover:bg-surface-muted/60">
      <Artwork artworkId={item.track.artwork_id} alt="" className="h-12 w-12" />
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-semibold text-copy">{item.track.title}</p>
        <p className="truncate text-xs text-copy-muted">{artists}</p>
      </div>
      <div className="text-right">
        <p className="text-xs font-semibold text-copy">{formatHistoryEvent(item.event_kind)}</p>
        <p className="text-[0.7rem] text-copy-muted">
          <time dateTime={item.occurred_at}>{formatDateTime(item.occurred_at)}</time>
          {showSeconds && <span className="ml-2 font-mono">{formatClock(item.seconds ?? 0)}</span>}
        </p>
      </div>
    </li>
  );
}

export function HistoryView() {
  const { client, expireSession } = useSession();

  const loadPage = useCallback(
    async (cursor: string | undefined, signal: AbortSignal) => {
      return client.listHistory({ limit: PAGE_SIZE, cursor }, signal);
    },
    [client],
  );

  const resource = usePagedResource<HistoryItem>(loadPage);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);

  useEffect(() => {
    if (resource.error?.status === 401) {
      expireSession();
    }
  }, [resource.error, expireSession]);

  const clearHistory = useCallback(async () => {
    setClearing(true);

    try {
      await client.clearHistory();
      setConfirmOpen(false);
      setFeedback('Histórico limpo.');
      resource.reload();
    } catch (cause) {
      const apiError = toApiError(cause);
      setConfirmOpen(false);
      setFeedback(describeApiError(apiError));

      if (apiError.status === 401) {
        expireSession();
      }
    } finally {
      setClearing(false);
    }
  }, [client, expireSession, resource]);

  const isEmpty = resource.status === 'ready' && resource.items.length === 0;

  return (
    <section aria-labelledby="history-heading">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="text-xs font-semibold tracking-[0.18em] text-accent-cyan uppercase">
            Sua conta
          </p>
          <h2 id="history-heading" className="mt-1 text-2xl font-semibold text-copy">
            Histórico
          </h2>
          <p className="mt-2 max-w-prose text-sm text-copy-muted">
            Reproduções registradas para você. Este histórico é privado e visível apenas para a sua
            conta; o envio para IA na nuvem depende do seu consentimento em{' '}
            <a href={routeHref({ name: 'settings' })} className="underline hover:text-copy">
              Configurações
            </a>
            .
          </p>
        </div>

        <button
          type="button"
          onClick={() => {
            setConfirmOpen(true);
          }}
          disabled={clearing || resource.status !== 'ready' || resource.items.length === 0}
          className="inline-flex min-h-11 items-center gap-2 rounded-xl border border-line px-4 py-2 text-sm font-semibold text-copy hover:bg-surface-muted disabled:cursor-not-allowed disabled:opacity-50"
        >
          <TrashIcon className="h-4 w-4" />
          Limpar histórico
        </button>
      </div>

      <div role="status" aria-live="polite" className="mt-4 min-h-5 text-sm text-copy-muted">
        {feedback ?? ''}
      </div>

      {resource.status === 'loading' && <LoadingSkeleton rows={5} label="Carregando histórico…" />}

      {resource.status === 'error' && resource.error && (
        <ErrorState message={describeApiError(resource.error)} onRetry={resource.reload} />
      )}

      {isEmpty && (
        <EmptyState
          title="Seu histórico está vazio"
          description="As faixas que você tocar aparecem aqui com o tipo de evento e o horário. O histórico é individual, privado e pode ser limpo a qualquer momento. Se você desativar o envio para IA na nuvem, nada sai desta instância."
        />
      )}

      {resource.status === 'ready' && resource.items.length > 0 && (
        <>
          <ol aria-label="Eventos do histórico" className="mt-2 divide-y divide-line/50">
            {resource.items.map((item) => (
              <HistoryRow key={item.id} item={item} />
            ))}
          </ol>

          {resource.nextCursor !== null && (
            <div className="mt-5 flex justify-center">
              <button
                type="button"
                onClick={resource.loadMore}
                disabled={resource.loadingMore}
                className="primary-button inline-flex min-h-11 items-center rounded-xl px-5 py-2 text-sm font-semibold disabled:cursor-not-allowed disabled:opacity-60"
              >
                {resource.loadingMore ? 'Carregando…' : 'Carregar mais'}
              </button>
            </div>
          )}
        </>
      )}

      {confirmOpen && (
        <Dialog
          title="Limpar todo o histórico?"
          description="Esta ação remove permanentemente todas as reproduções registradas na sua conta. Não é possível desfazer."
          onClose={() => {
            if (!clearing) {
              setConfirmOpen(false);
            }
          }}
          footer={
            <>
              <button
                type="button"
                data-autofocus
                onClick={() => {
                  setConfirmOpen(false);
                }}
                disabled={clearing}
                className="inline-flex min-h-11 items-center rounded-xl border border-line px-4 py-2 text-sm font-semibold text-copy hover:bg-surface-muted disabled:opacity-60"
              >
                Cancelar
              </button>
              <button
                type="button"
                onClick={() => {
                  void clearHistory();
                }}
                disabled={clearing}
                className="inline-flex min-h-11 items-center gap-2 rounded-xl bg-red-600 px-4 py-2 text-sm font-semibold text-white hover:bg-red-700 disabled:opacity-60"
              >
                <TrashIcon className="h-4 w-4" />
                {clearing ? 'Limpando…' : 'Sim, limpar histórico'}
              </button>
            </>
          }
        >
          <p className="text-sm text-copy-muted">
            Preferências de privacidade e retenção não são alteradas. Para mudar a retenção
            automática, use as Configurações.
          </p>
        </Dialog>
      )}
    </section>
  );
}
