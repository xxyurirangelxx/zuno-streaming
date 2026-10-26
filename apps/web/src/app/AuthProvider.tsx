import { ZunoApiError } from '@zuno/api-client';
import type { ZunoClient } from '@zuno/api-client';
import type {
  LoginRequest,
  RedeemInvitationRequest,
  SetupClaimRequest,
  User,
} from '@zuno/shared-types';
import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';

export type SessionState =
  | { kind: 'loading' }
  | { kind: 'setup' }
  | { kind: 'login' }
  | { kind: 'authenticated'; user: User }
  | { kind: 'error'; error: ZunoApiError };

export interface SessionContextValue {
  client: ZunoClient;
  state: SessionState;
  login: (credentials: LoginRequest) => Promise<void>;
  setupClaim: (payload: SetupClaimRequest) => Promise<void>;
  redeemInvitation: (payload: RedeemInvitationRequest) => Promise<void>;
  logout: () => Promise<void>;
  retry: () => void;
  /** Chamado quando uma requisição autenticada falha com 401. */
  expireSession: () => void;
}

const SessionContext = createContext<SessionContextValue | null>(null);

function toApiError(error: unknown): ZunoApiError {
  if (error instanceof ZunoApiError) {
    return error;
  }

  return new ZunoApiError('unknown', 'Ocorreu uma falha inesperada.', null);
}

export function AuthProvider({ client, children }: { client: ZunoClient; children: ReactNode }) {
  const [state, setState] = useState<SessionState>({ kind: 'loading' });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    const controller = new AbortController();

    async function bootstrap() {
      setState({ kind: 'loading' });

      try {
        const user = await client.me(controller.signal);

        if (!controller.signal.aborted) {
          setState({ kind: 'authenticated', user });
        }
      } catch (error) {
        if (controller.signal.aborted) {
          return;
        }

        if (error instanceof ZunoApiError && error.status === 401) {
          try {
            const status = await client.setupStatus(controller.signal);

            if (!controller.signal.aborted) {
              setState(status.required ? { kind: 'setup' } : { kind: 'login' });
            }
          } catch (statusError) {
            if (!controller.signal.aborted) {
              setState({ kind: 'error', error: toApiError(statusError) });
            }
          }

          return;
        }

        setState({ kind: 'error', error: toApiError(error) });
      }
    }

    void bootstrap();

    return () => {
      controller.abort();
    };
  }, [client, attempt]);

  const login = useCallback(
    async (credentials: LoginRequest) => {
      const session = await client.login(credentials);
      setState({ kind: 'authenticated', user: session.user });
    },
    [client],
  );

  const setupClaim = useCallback(
    async (payload: SetupClaimRequest) => {
      const session = await client.setupClaim(payload);
      setState({ kind: 'authenticated', user: session.user });
    },
    [client],
  );

  const redeemInvitation = useCallback(
    async (payload: RedeemInvitationRequest) => {
      const session = await client.redeemInvitation(payload);
      setState({ kind: 'authenticated', user: session.user });
    },
    [client],
  );

  const logout = useCallback(async () => {
    try {
      await client.logout();
    } catch {
      // Mesmo se a revogação falhar, a UI volta ao login: o cookie pode estar
      // inválido e uma nova tentativa é oferecida no formulário.
    } finally {
      setState({ kind: 'login' });
    }
  }, [client]);

  const expireSession = useCallback(() => {
    setState((current) => (current.kind === 'authenticated' ? { kind: 'login' } : current));
  }, []);

  const retry = useCallback(() => {
    setAttempt((value) => value + 1);
  }, []);

  const value = useMemo<SessionContextValue>(
    () => ({
      client,
      state,
      login,
      setupClaim,
      redeemInvitation,
      logout,
      retry,
      expireSession,
    }),
    [client, state, login, setupClaim, redeemInvitation, logout, retry, expireSession],
  );

  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>;
}

export function useSession(): SessionContextValue {
  const context = useContext(SessionContext);

  if (!context) {
    throw new Error('useSession precisa estar dentro de AuthProvider.');
  }

  return context;
}
