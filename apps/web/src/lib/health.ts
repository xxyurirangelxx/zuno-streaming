import { ZunoApiError } from '@zuno/api-client';
import type { ZunoClient } from '@zuno/api-client';
import type { HealthResponse } from '@zuno/shared-types';
import { useCallback, useEffect, useRef, useState } from 'react';

export type HealthViewState =
  | { kind: 'loading' }
  | { kind: 'ok'; data: HealthResponse; checkedAt: Date }
  | { kind: 'degraded'; data: HealthResponse; checkedAt: Date }
  | { kind: 'error'; error: ZunoApiError; checkedAt: Date };

export interface UseHealthResult {
  state: HealthViewState;
  refresh: () => void;
}

function toApiError(error: unknown): ZunoApiError {
  if (error instanceof ZunoApiError) {
    return error;
  }

  return new ZunoApiError(
    'network_error',
    'Ocorreu uma falha inesperada ao consultar o servidor.',
    null,
  );
}

function toSuccessState(data: HealthResponse): HealthViewState {
  const checkedAt = new Date();

  return data.status === 'ok' && data.database.status === 'ok'
    ? { kind: 'ok', data, checkedAt }
    : { kind: 'degraded', data, checkedAt };
}

export function useHealth(client: ZunoClient): UseHealthResult {
  const [state, setState] = useState<HealthViewState>({ kind: 'loading' });
  const activeController = useRef<AbortController | null>(null);

  const settle = useCallback((controller: AbortController, request: Promise<HealthResponse>) => {
    void request
      .then((data) => {
        if (!controller.signal.aborted) {
          setState(toSuccessState(data));
        }
      })
      .catch((error: unknown) => {
        if (!controller.signal.aborted) {
          setState({ kind: 'error', error: toApiError(error), checkedAt: new Date() });
        }
      });
  }, []);

  const startRequest = useCallback(() => {
    activeController.current?.abort();

    const controller = new AbortController();
    activeController.current = controller;
    setState({ kind: 'loading' });
    settle(controller, client.health(controller.signal));
  }, [client, settle]);

  useEffect(() => {
    const controller = new AbortController();
    activeController.current = controller;
    settle(controller, client.health(controller.signal));

    return () => {
      controller.abort();
    };
  }, [client, settle]);

  return { state, refresh: startRequest };
}
