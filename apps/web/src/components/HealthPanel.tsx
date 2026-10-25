import type { HealthViewState } from '../lib/health';
import {
  describeHealthError,
  formatCheckedAt,
  formatDatabaseStatus,
  formatHealthStatus,
  formatUptime,
} from '../lib/format';

interface HealthPanelProps {
  state: HealthViewState;
  onRefresh: () => void;
}

interface DetailProps {
  label: string;
  value: string;
}

function Detail({ label, value }: DetailProps) {
  return (
    <div className="rounded-xl border border-line/70 bg-surface/60 px-4 py-3">
      <dt className="text-xs font-semibold tracking-wider text-copy-muted uppercase">{label}</dt>
      <dd className="mt-1 text-sm font-medium break-words text-copy">{value}</dd>
    </div>
  );
}

function StatusDot({ tone }: { tone: 'ok' | 'degraded' | 'error' | 'loading' }) {
  const toneClass = {
    ok: 'bg-emerald-500',
    degraded: 'bg-amber-500',
    error: 'bg-red-500',
    loading: 'bg-accent-blue',
  }[tone];

  return (
    <span
      aria-hidden="true"
      className={`inline-block h-2.5 w-2.5 shrink-0 rounded-full ${toneClass} ${
        tone === 'loading' ? 'motion-safe:animate-pulse' : ''
      }`}
    />
  );
}

function LoadingState() {
  return (
    <div className="flex items-start gap-3">
      <StatusDot tone="loading" />
      <div className="min-w-0 flex-1">
        <p className="font-medium text-copy">Consultando a saúde do servidor…</p>
        <div className="mt-3 space-y-2" aria-hidden="true">
          <div className="h-2 w-2/3 rounded-full bg-surface-muted motion-safe:animate-pulse" />
          <div className="h-2 w-1/3 rounded-full bg-surface-muted motion-safe:animate-pulse" />
        </div>
      </div>
    </div>
  );
}

export function HealthPanel({ state, onRefresh }: HealthPanelProps) {
  const isLoading = state.kind === 'loading';

  return (
    <section
      aria-labelledby="health-heading"
      className="glass-panel w-full rounded-zuno p-6 sm:p-8"
    >
      <div className="flex flex-col gap-5 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <p className="text-xs font-semibold tracking-[0.18em] text-accent-cyan uppercase">
            Diagnóstico
          </p>
          <h2 id="health-heading" className="mt-1 text-2xl font-semibold text-copy">
            Saúde do servidor
          </h2>
          <p className="mt-2 max-w-prose text-sm text-copy-muted">
            Consulta <code className="font-mono text-copy">GET /health</code> para confirmar a
            conexão entre a interface e o backend local.
          </p>
        </div>

        <button
          type="button"
          onClick={onRefresh}
          disabled={isLoading}
          className="primary-button inline-flex min-h-11 shrink-0 items-center justify-center rounded-xl px-5 py-2.5 text-sm font-semibold disabled:cursor-not-allowed disabled:opacity-70"
        >
          {isLoading ? 'Verificando…' : 'Verificar novamente'}
        </button>
      </div>

      <div role="status" aria-live="polite" aria-busy={isLoading} className="mt-6">
        {state.kind === 'loading' && <LoadingState />}

        {(state.kind === 'ok' || state.kind === 'degraded') && (
          <div>
            <div className="flex flex-wrap items-center gap-3">
              <StatusDot tone={state.kind} />
              <h3 className="text-lg font-semibold text-copy">
                Servidor {formatHealthStatus(state.data.status)}
              </h3>
              {state.kind === 'degraded' && (
                <span className="rounded-full border border-line bg-surface-muted px-3 py-1 text-xs font-semibold text-copy">
                  Banco de dados indisponível
                </span>
              )}
            </div>

            {state.kind === 'degraded' && (
              <p className="mt-2 text-sm text-copy-muted">
                O banco de dados não está respondendo. Verifique os logs do servidor e se o arquivo
                SQLite está acessível.
              </p>
            )}

            <dl className="mt-5 grid grid-cols-1 gap-3 sm:grid-cols-2">
              <Detail label="Serviço" value={state.data.service} />
              <Detail label="Versão" value={state.data.version} />
              <Detail label="Tempo ativo" value={formatUptime(state.data.uptime_seconds)} />
              <Detail
                label="Banco de dados"
                value={formatDatabaseStatus(state.data.database.status)}
              />
              <Detail label="Última verificação" value={formatCheckedAt(state.checkedAt)} />
            </dl>
          </div>
        )}

        {state.kind === 'error' && (
          <div className="flex items-start gap-3">
            <StatusDot tone="error" />
            <div>
              <h3 className="text-lg font-semibold text-copy">
                Não foi possível verificar o servidor
              </h3>
              <p className="mt-1 text-sm text-copy-muted">{describeHealthError(state.error)}</p>
              <p className="mt-3 text-xs text-copy-muted">
                Código: <code className="font-mono text-copy">{state.error.code}</code> · Última
                tentativa: {formatCheckedAt(state.checkedAt)}
              </p>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
