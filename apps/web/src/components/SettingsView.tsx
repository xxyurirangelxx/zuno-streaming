import { ZunoApiError } from '@zuno/api-client';
import type { ThemePreference, UpdatePrivacyRequest, UserPrivacy } from '@zuno/shared-types';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useSession } from '../app/AuthProvider';
import { describeApiError } from '../lib/errors';
import { usePreferences } from '../theme/ThemeProvider';
import { ErrorState, LoadingSkeleton } from './States';

type PrivacyStatus = 'loading' | 'ready' | 'error';

interface Feedback {
  kind: 'success' | 'error';
  message: string;
}

const THEME_OPTIONS: { value: ThemePreference; label: string; description: string }[] = [
  {
    value: 'system',
    label: 'Seguir o sistema',
    description: 'Usa o tema claro ou escuro configurado no dispositivo.',
  },
  { value: 'light', label: 'Claro', description: 'Sempre usa a paleta clara.' },
  { value: 'dark', label: 'Escuro', description: 'Sempre usa a paleta escura.' },
];

const RETENTION_OPTIONS: { value: string; label: string }[] = [
  { value: 'never', label: 'Nunca' },
  { value: '30', label: '30 dias' },
  { value: '90', label: '90 dias' },
  { value: '365', label: '365 dias' },
];

function toApiError(error: unknown): ZunoApiError {
  if (error instanceof ZunoApiError) {
    return error;
  }

  return new ZunoApiError('unknown', 'Ocorreu uma falha inesperada.', null);
}

export function SettingsView() {
  const { client, expireSession } = useSession();
  const {
    preferences,
    status: preferencesStatus,
    error: preferencesError,
    setTheme,
    setLanguage,
    reload: reloadPreferences,
  } = usePreferences();

  const [privacy, setPrivacy] = useState<UserPrivacy | null>(null);
  const [privacyStatus, setPrivacyStatus] = useState<PrivacyStatus>('loading');
  const [privacyError, setPrivacyError] = useState<ZunoApiError | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const privacyRef = useRef<UserPrivacy | null>(null);

  const commitPrivacy = useCallback((next: UserPrivacy) => {
    privacyRef.current = next;
    setPrivacy(next);
  }, []);

  useEffect(() => {
    const controller = new AbortController();

    async function load() {
      try {
        const response = await client.getPrivacy(controller.signal);

        if (controller.signal.aborted) {
          return;
        }

        commitPrivacy(response);
        setPrivacyStatus('ready');
        setPrivacyError(null);
      } catch (cause) {
        if (controller.signal.aborted) {
          return;
        }

        const apiError = toApiError(cause);
        setPrivacyStatus('error');
        setPrivacyError(apiError);

        if (apiError.status === 401) {
          expireSession();
        }
      }
    }

    void load();

    return () => {
      controller.abort();
    };
  }, [client, commitPrivacy, expireSession, reloadToken]);

  const persistPrivacy = useCallback(
    async (patch: UpdatePrivacyRequest, successMessage: string) => {
      const previous = privacyRef.current;

      if (!previous) {
        return;
      }

      commitPrivacy({ ...previous, ...patch });

      try {
        const saved = await client.updatePrivacy(patch);
        commitPrivacy(saved);
        setFeedback({ kind: 'success', message: successMessage });
      } catch (cause) {
        commitPrivacy(previous);
        const apiError = toApiError(cause);
        setFeedback({ kind: 'error', message: describeApiError(apiError) });

        if (apiError.status === 401) {
          expireSession();
        }
      }
    },
    [client, commitPrivacy, expireSession],
  );

  const retryPrivacy = useCallback(() => {
    setPrivacyStatus('loading');
    setPrivacyError(null);
    setReloadToken((token) => token + 1);
  }, []);

  const retentionValue =
    privacy?.history_retention_days === null || privacy === null
      ? 'never'
      : String(privacy.history_retention_days);

  return (
    <div className="space-y-8">
      <header>
        <p className="text-xs font-semibold tracking-[0.18em] text-accent-cyan uppercase">
          Sua conta
        </p>
        <h2 className="mt-1 text-2xl font-semibold text-copy">Configurações</h2>
        <p className="mt-2 max-w-prose text-sm text-copy-muted">
          Ajustes de aparência e privacidade salvos no seu perfil. As preferências valem para todos
          os dispositivos conectados a esta instância.
        </p>
      </header>

      <section aria-labelledby="appearance-heading" className="glass-panel rounded-zuno p-5 sm:p-6">
        <h3 id="appearance-heading" className="text-lg font-semibold text-copy">
          Aparência
        </h3>

        <fieldset className="mt-4" disabled={preferencesStatus === 'loading'}>
          <legend className="text-sm font-semibold text-copy">Tema</legend>
          <div className="mt-2 grid gap-2 sm:grid-cols-3">
            {THEME_OPTIONS.map((option) => {
              const inputId = `theme-${option.value}`;

              return (
                <label
                  key={option.value}
                  htmlFor={inputId}
                  className="flex cursor-pointer items-start gap-3 rounded-xl border border-line/70 bg-surface/50 p-3 has-[:checked]:border-accent-blue has-[:checked]:bg-accent-blue/10"
                >
                  <input
                    id={inputId}
                    type="radio"
                    name="theme"
                    value={option.value}
                    checked={preferences.theme === option.value}
                    onChange={() => {
                      void setTheme(option.value);
                    }}
                    className="mt-1 h-4 w-4 accent-accent-blue"
                  />
                  <span>
                    <span className="block text-sm font-semibold text-copy">{option.label}</span>
                    <span className="mt-0.5 block text-xs text-copy-muted">
                      {option.description}
                    </span>
                  </span>
                </label>
              );
            })}
          </div>
        </fieldset>

        {preferencesStatus === 'error' && preferencesError && (
          <div className="mt-4">
            <ErrorState
              title="Não foi possível carregar suas preferências"
              message={`${describeApiError(preferencesError)} O tema do sistema continua ativo.`}
              onRetry={reloadPreferences}
            />
          </div>
        )}

        <div className="mt-5">
          <label htmlFor="language" className="text-sm font-semibold text-copy">
            Idioma
          </label>
          <select
            id="language"
            value={preferences.language}
            onChange={(event) => {
              void setLanguage(event.target.value);
            }}
            className="mt-2 block min-h-11 w-full max-w-xs rounded-xl border border-line bg-surface px-3 text-sm text-copy"
          >
            <option value="pt-BR">Português (Brasil) — pt-BR</option>
            <option value="en-US" disabled>
              English — em breve
            </option>
          </select>
          <p className="mt-2 text-xs text-copy-muted">
            Mais idiomas chegam em uma versão futura. O inglês está em breve.
          </p>
        </div>

        <p className="mt-5 rounded-xl border border-line/60 bg-surface-muted/60 p-3 text-xs text-copy-muted">
          Redução de movimento, transparência e alto contraste seguem as preferências do sistema
          operacional. O Zuno respeita <code>prefers-reduced-motion</code>,{' '}
          <code>prefers-reduced-transparency</code> e <code>prefers-contrast</code>.
        </p>
      </section>

      <section aria-labelledby="privacy-heading" className="glass-panel rounded-zuno p-5 sm:p-6">
        <h3 id="privacy-heading" className="text-lg font-semibold text-copy">
          Privacidade
        </h3>
        <p className="mt-2 max-w-prose text-sm text-copy-muted">
          Seu histórico é visível apenas para você. O envio para IA na nuvem é opcional e pode ser
          vetado a qualquer momento; biblioteca, reprodução e recomendações locais continuam
          funcionando sem nuvem. A integração de IA chega no M7.
        </p>

        {privacyStatus === 'loading' && (
          <div className="mt-4">
            <LoadingSkeleton rows={2} label="Carregando preferências de privacidade…" />
          </div>
        )}

        {privacyStatus === 'error' && privacyError && (
          <div className="mt-4">
            <ErrorState
              title="Não foi possível carregar sua privacidade"
              message={describeApiError(privacyError)}
              onRetry={retryPrivacy}
            />
          </div>
        )}

        {privacyStatus === 'ready' && privacy && (
          <div className="mt-4 space-y-5">
            <div className="flex items-start gap-3 rounded-xl border border-line/70 bg-surface/50 p-4">
              <input
                id="allow-cloud-history"
                type="checkbox"
                checked={privacy.allow_cloud_history}
                onChange={(event) => {
                  const allow = event.target.checked;
                  void persistPrivacy(
                    { allow_cloud_history: allow },
                    allow
                      ? 'Envio do histórico para IA na nuvem permitido.'
                      : 'Envio do histórico para IA na nuvem bloqueado.',
                  );
                }}
                className="mt-0.5 h-5 w-5 shrink-0 accent-accent-blue"
              />
              <label htmlFor="allow-cloud-history" className="text-sm">
                <span className="block font-semibold text-copy">
                  Permitir enviar meu histórico para IA na nuvem
                </span>
                <span className="mt-1 block text-xs text-copy-muted">
                  Desativado por padrão. Sem permissão, nenhum evento do seu histórico sai desta
                  instância.
                </span>
              </label>
            </div>

            <div>
              <label htmlFor="history-retention" className="text-sm font-semibold text-copy">
                Retenção do histórico
              </label>
              <select
                id="history-retention"
                value={retentionValue}
                onChange={(event) => {
                  const value = event.target.value;
                  const days = value === 'never' ? null : Number(value);
                  void persistPrivacy(
                    { history_retention_days: days },
                    days === null
                      ? 'Histórico mantido até você limpá-lo.'
                      : `Histórico mantido por ${days} dias.`,
                  );
                }}
                className="mt-2 block min-h-11 w-full max-w-xs rounded-xl border border-line bg-surface px-3 text-sm text-copy"
              >
                {RETENTION_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
              <p className="mt-2 text-xs text-copy-muted">
                “Nunca” não apaga automaticamente: o histórico fica até você usar “Limpar histórico”
                na página Histórico. A retenção é aplicada pelo servidor.
              </p>
            </div>
          </div>
        )}

        <div
          role={feedback?.kind === 'error' ? 'alert' : 'status'}
          aria-live={feedback?.kind === 'error' ? 'assertive' : 'polite'}
          className="mt-4 min-h-5 text-sm text-copy-muted"
        >
          {feedback?.message ?? ''}
        </div>
      </section>
    </div>
  );
}
