import type { TrackSummary } from '@zuno/shared-types';
import { useCallback, useEffect } from 'react';
import { useSession } from '../app/AuthProvider';
import { describeApiError } from '../lib/errors';
import { routeHref } from '../lib/router';
import { usePagedResource } from '../lib/usePagedResource';
import { EmptyState, ErrorState, LoadingSkeleton } from './States';
import { TrackList } from './TrackList';

function ShortcutCard({
  href,
  title,
  description,
}: {
  href: string;
  title: string;
  description: string;
}) {
  return (
    <a
      href={href}
      className="glass-panel block rounded-zuno p-5 transition-colors hover:border-accent-blue/60"
    >
      <p className="text-base font-semibold text-copy">{title}</p>
      <p className="mt-1 text-sm text-copy-muted">{description}</p>
    </a>
  );
}

export function HomeView() {
  const { client, state: sessionState, expireSession, libraryRevision } = useSession();
  const user = sessionState.kind === 'authenticated' ? sessionState.user : null;

  const loadPage = useCallback(
    async (_cursor: string | undefined, signal: AbortSignal) => {
      // Referenciado para invalidar a identidade do callback quando houver upload.
      void libraryRevision;
      const page = await client.listTracks({ limit: 8 }, signal);
      return { items: page.items, next_cursor: null };
    },
    [client, libraryRevision],
  );

  const recent = usePagedResource<TrackSummary>(loadPage);

  useEffect(() => {
    if (recent.error?.status === 401) {
      expireSession();
    }
  }, [recent.error, expireSession]);

  return (
    <div className="space-y-8">
      <section aria-labelledby="home-heading" className="glass-panel rounded-zuno p-6 sm:p-8">
        <p className="text-xs font-semibold tracking-[0.18em] text-accent-cyan uppercase">Início</p>
        <h2 id="home-heading" className="mt-1 text-3xl font-semibold tracking-tight text-copy">
          Olá, {user?.username ?? 'ouvinte'}
        </h2>
        <p className="mt-2 max-w-prose text-sm text-copy-muted">
          Sua biblioteca pessoal, sem anúncios e sem telemetria. Comece pelas faixas recentes,
          explore álbuns ou encontre algo novo na busca.
        </p>
      </section>

      <section aria-labelledby="shortcuts-heading">
        <h3 id="shortcuts-heading" className="mb-3 text-lg font-semibold text-copy">
          Atalhos
        </h3>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <ShortcutCard
            href={routeHref({ name: 'library', kind: 'albums' })}
            title="Álbuns"
            description="Navegue pela coleção por álbum e capa."
          />
          <ShortcutCard
            href={routeHref({ name: 'search' })}
            title="Buscar"
            description="Encontre faixas, artistas e álbuns rapidamente."
          />
        </div>
      </section>

      <section aria-labelledby="recent-heading">
        <h3 id="recent-heading" className="mb-3 text-lg font-semibold text-copy">
          Recentes
        </h3>

        {recent.status === 'loading' && <LoadingSkeleton rows={4} label="Carregando faixas…" />}

        {recent.status === 'error' && recent.error && (
          <ErrorState message={describeApiError(recent.error)} onRetry={recent.reload} />
        )}

        {recent.status === 'ready' &&
          (recent.items.length === 0 ? (
            <EmptyState
              title="Biblioteca vazia"
              description="Envie arquivos Opus para que suas faixas apareçam aqui."
            />
          ) : (
            <TrackList tracks={recent.items} />
          ))}
      </section>
    </div>
  );
}
