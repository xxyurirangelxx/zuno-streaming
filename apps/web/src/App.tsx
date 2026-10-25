import { createZunoClient } from '@zuno/api-client';
import { useMemo } from 'react';
import { HealthPanel } from './components/HealthPanel';
import { useHealth } from './lib/health';

const defaultBaseUrl = import.meta.env.VITE_ZUNO_API_BASE_URL ?? '';

export default function App() {
  const client = useMemo(() => createZunoClient({ baseUrl: defaultBaseUrl }), []);
  const health = useHealth(client);

  return (
    <div className="flex min-h-screen flex-col">
      <header className="mx-auto flex w-full max-w-5xl items-center gap-4 px-5 pt-8 sm:px-8 sm:pt-10">
        <img
          src="/logo.png"
          alt=""
          className="h-14 w-auto max-w-40 shrink-0 object-contain sm:h-16"
        />
        <div className="min-w-0">
          <h1 className="text-3xl font-semibold tracking-tight text-copy">Zuno</h1>
          <p className="text-sm text-copy-muted">Fundação técnica (M0)</p>
        </div>
      </header>

      <main className="mx-auto flex w-full max-w-3xl flex-1 items-start px-5 py-10 sm:px-8 sm:py-16">
        <HealthPanel state={health.state} onRefresh={health.refresh} />
      </main>

      <footer className="mx-auto w-full max-w-5xl px-5 pb-8 sm:px-8">
        <p className="border-t border-line/70 pt-5 text-xs leading-relaxed text-copy-muted">
          M0 — sem autenticação, biblioteca ou streaming. Licença ainda não definida.
        </p>
      </footer>
    </div>
  );
}
