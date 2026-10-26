import { ZunoApiError } from '@zuno/api-client';
import { useCallback, useEffect, useState } from 'react';

export interface Page<T> {
  items: T[];
  next_cursor: string | null;
}

export type LoadPage<T> = (cursor: string | undefined, signal: AbortSignal) => Promise<Page<T>>;

export type ResourceStatus = 'loading' | 'ready' | 'error';

export interface PagedResource<T> {
  items: T[];
  nextCursor: string | null;
  status: ResourceStatus;
  error: ZunoApiError | null;
  loadingMore: boolean;
  loadMore: () => void;
  reload: () => void;
}

interface Settled<T> {
  loadPage: LoadPage<T>;
  token: number;
  items: T[];
  nextCursor: string | null;
  status: 'ready' | 'error';
  error: ZunoApiError | null;
}

function toApiError(error: unknown): ZunoApiError {
  if (error instanceof ZunoApiError) {
    return error;
  }

  return new ZunoApiError('unknown', 'Ocorreu uma falha inesperada.', null);
}

/**
 * Paginação por `next_cursor` sem setState síncrono no efeito: o estado
 * guarda a origem da última resposta e o status é derivado dela.
 */
export function usePagedResource<T>(loadPage: LoadPage<T>): PagedResource<T> {
  const [settled, setSettled] = useState<Settled<T> | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const [loadingMore, setLoadingMore] = useState(false);

  useEffect(() => {
    const controller = new AbortController();
    const token = reloadToken;

    loadPage(undefined, controller.signal)
      .then((page) => {
        if (controller.signal.aborted) {
          return;
        }

        setSettled({
          loadPage,
          token,
          items: page.items,
          nextCursor: page.next_cursor,
          status: 'ready',
          error: null,
        });
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) {
          return;
        }

        setSettled({
          loadPage,
          token,
          items: [],
          nextCursor: null,
          status: 'error',
          error: toApiError(cause),
        });
      });

    return () => {
      controller.abort();
    };
  }, [loadPage, reloadToken]);

  const stale = settled === null || settled.loadPage !== loadPage || settled.token !== reloadToken;

  const loadMore = useCallback(() => {
    if (stale || settled === null || settled.nextCursor === null || loadingMore) {
      return;
    }

    const cursor = settled.nextCursor;
    setLoadingMore(true);

    loadPage(cursor, new AbortController().signal)
      .then((page) => {
        setSettled((current) => {
          if (!current || current.loadPage !== loadPage) {
            return current;
          }

          return {
            ...current,
            items: [...current.items, ...page.items],
            nextCursor: page.next_cursor,
          };
        });
      })
      .catch((cause: unknown) => {
        setSettled((current) => {
          if (!current || current.loadPage !== loadPage) {
            return current;
          }

          return { ...current, status: 'error', error: toApiError(cause) };
        });
      })
      .finally(() => {
        setLoadingMore(false);
      });
  }, [loadPage, loadingMore, settled, stale]);

  const reload = useCallback(() => {
    setReloadToken((token) => token + 1);
  }, []);

  if (stale || settled === null) {
    return {
      items: [],
      nextCursor: null,
      status: 'loading',
      error: null,
      loadingMore,
      loadMore,
      reload,
    };
  }

  return {
    items: settled.items,
    nextCursor: settled.nextCursor,
    status: settled.status,
    error: settled.error,
    loadingMore,
    loadMore,
    reload,
  };
}
