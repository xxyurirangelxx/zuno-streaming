import { ZunoApiError } from '@zuno/api-client';
import type { ZunoClient } from '@zuno/api-client';
import type {
  ThemePreference,
  UpdatePreferencesRequest,
  UserPreferences,
} from '@zuno/shared-types';
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import type { ReactNode } from 'react';
import { describeApiError } from '../lib/errors';

export type PreferencesStatus = 'loading' | 'ready' | 'error';

interface Feedback {
  kind: 'success' | 'error';
  message: string;
}

export interface PreferencesContextValue {
  preferences: UserPreferences;
  status: PreferencesStatus;
  error: ZunoApiError | null;
  feedback: Feedback | null;
  setTheme: (theme: ThemePreference) => Promise<void>;
  setLanguage: (language: string) => Promise<void>;
  reload: () => void;
}

const DEFAULT_PREFERENCES: UserPreferences = { theme: 'system', language: 'pt-BR' };

const PreferencesContext = createContext<PreferencesContextValue | null>(null);

function toApiError(error: unknown): ZunoApiError {
  if (error instanceof ZunoApiError) {
    return error;
  }

  return new ZunoApiError('unknown', 'Ocorreu uma falha inesperada.', null);
}

/** Aplica o tema no documento; `system` remove o override e segue o SO. */
export function applyTheme(theme: ThemePreference): void {
  const root = document.documentElement;

  if (theme === 'system') {
    root.removeAttribute('data-theme');
    return;
  }

  root.setAttribute('data-theme', theme);
}

export function ThemeProvider({ client, children }: { client: ZunoClient; children: ReactNode }) {
  const [preferences, setPreferences] = useState<UserPreferences>(DEFAULT_PREFERENCES);
  const [initialized, setInitialized] = useState(false);
  const [status, setStatus] = useState<PreferencesStatus>('loading');
  const [error, setError] = useState<ZunoApiError | null>(null);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const preferencesRef = useRef<UserPreferences>(DEFAULT_PREFERENCES);
  const mutationRef = useRef(0);

  const commit = useCallback((next: UserPreferences) => {
    preferencesRef.current = next;
    setPreferences(next);
  }, []);

  // Aplicado antes da pintura: evita flash de tema ao entrar ou trocar a preferência.
  useLayoutEffect(() => {
    applyTheme(preferences.theme);
  }, [preferences.theme]);

  useEffect(() => {
    const controller = new AbortController();

    async function load() {
      try {
        const response = await client.getPreferences(controller.signal);

        if (controller.signal.aborted) {
          return;
        }

        commit(response);
        setStatus('ready');
        setError(null);
      } catch (cause) {
        if (controller.signal.aborted) {
          return;
        }

        // Falha ao carregar preferências não bloqueia o app: segue o sistema.
        commit(DEFAULT_PREFERENCES);
        setStatus('error');
        setError(toApiError(cause));
      } finally {
        if (!controller.signal.aborted) {
          setInitialized(true);
        }
      }
    }

    void load();

    return () => {
      controller.abort();
    };
  }, [client, commit, reloadToken]);

  useEffect(() => {
    return () => {
      // Ao sair da sessão, a tela de login volta a seguir o sistema operacional.
      document.documentElement.removeAttribute('data-theme');
    };
  }, []);

  const persist = useCallback(
    async (patch: UpdatePreferencesRequest) => {
      const sequence = mutationRef.current + 1;
      mutationRef.current = sequence;
      const previous = preferencesRef.current;

      commit({ ...previous, ...patch });

      try {
        const saved = await client.updatePreferences(patch);

        if (sequence !== mutationRef.current) {
          return;
        }

        commit(saved);
        setFeedback({ kind: 'success', message: 'Preferências salvas.' });
      } catch (cause) {
        if (sequence !== mutationRef.current) {
          return;
        }

        commit(previous);
        setFeedback({ kind: 'error', message: describeApiError(toApiError(cause)) });
      }
    },
    [client, commit],
  );

  const setTheme = useCallback(
    async (theme: ThemePreference) => {
      await persist({ theme });
    },
    [persist],
  );

  const setLanguage = useCallback(
    async (language: string) => {
      await persist({ language });
    },
    [persist],
  );

  const reload = useCallback(() => {
    setStatus('loading');
    setError(null);
    setReloadToken((token) => token + 1);
  }, []);

  const value = useMemo<PreferencesContextValue>(
    () => ({
      preferences,
      status,
      error,
      feedback,
      setTheme,
      setLanguage,
      reload,
    }),
    [preferences, status, error, feedback, setTheme, setLanguage, reload],
  );

  return (
    <PreferencesContext.Provider value={value}>
      {initialized ? children : null}
      <div
        role={feedback?.kind === 'error' ? 'alert' : 'status'}
        aria-live={feedback?.kind === 'error' ? 'assertive' : 'polite'}
        className="sr-only"
      >
        {feedback?.message ?? ''}
      </div>
    </PreferencesContext.Provider>
  );
}

export function usePreferences(): PreferencesContextValue {
  const context = useContext(PreferencesContext);

  if (!context) {
    throw new Error('usePreferences precisa estar dentro de ThemeProvider.');
  }

  return context;
}
