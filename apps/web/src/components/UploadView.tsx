import { useCallback, useId, useState } from 'react';
import { useSession } from '../app/AuthProvider';
import { routeHref } from '../lib/router';
import { describeUploadError } from '../lib/upload-errors';
import type { UploadItemStatus } from '../upload/useUploadQueue';
import { useUploadQueue } from '../upload/useUploadQueue';

const STATUS_LABELS: Record<UploadItemStatus, string> = {
  queued: 'Aguardando',
  sending: 'Enviando',
  validating: 'Validando',
  indexed: 'Indexado',
  duplicate: 'Duplicado',
  error: 'Erro',
  canceled: 'Cancelado',
};

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) {
    return '—';
  }

  if (bytes < 1024) {
    return `${bytes} B`;
  }

  const units = ['kB', 'MB', 'GB'];
  let value = bytes / 1024;
  let unit = units[0];

  for (let index = 1; index < units.length && value >= 1024; index += 1) {
    value /= 1024;
    unit = units[index];
  }

  return `${value.toFixed(value >= 10 ? 0 : 1).replace('.', ',')} ${unit}`;
}

function statusTone(status: UploadItemStatus): string {
  switch (status) {
    case 'indexed':
      return 'text-accent-cyan';
    case 'duplicate':
      return 'text-accent-blue';
    case 'error':
      return 'text-red-300';
    case 'canceled':
      return 'text-copy-muted';
    default:
      return 'text-copy';
  }
}

export function UploadView() {
  const { client, notifyLibraryChanged, expireSession } = useSession();
  const [dragging, setDragging] = useState(false);
  const inputId = useId();
  const hintId = useId();

  const handleIndexed = useCallback(() => {
    notifyLibraryChanged();
  }, [notifyLibraryChanged]);

  const handleUnauthorized = useCallback(() => {
    expireSession();
  }, [expireSession]);

  const queue = useUploadQueue({
    client,
    onIndexed: handleIndexed,
    onUnauthorized: handleUnauthorized,
  });

  const busy = queue.items.some(
    (item) => item.status === 'sending' || item.status === 'validating',
  );
  const hasIndexed = queue.items.some((item) => item.status === 'indexed');
  const hasClearable = queue.items.some(
    (item) =>
      item.status === 'indexed' || item.status === 'duplicate' || item.status === 'canceled',
  );

  return (
    <section aria-labelledby="upload-heading" aria-busy={busy} className="space-y-6">
      <div>
        <p className="text-xs font-semibold tracking-[0.18em] text-accent-cyan uppercase">Enviar</p>
        <h2 id="upload-heading" className="mt-1 text-2xl font-semibold text-copy">
          Enviar músicas
        </h2>
        <p className="mt-2 max-w-prose text-sm text-copy-muted">
          A biblioteca do Zuno é somente Opus. A conversão não acontece no navegador: converta seus
          arquivos para Opus 128 kbps VBR no cliente Windows/Tauri (M4) antes de enviar.
        </p>
      </div>

      <label
        htmlFor={inputId}
        onDragOver={(event) => {
          event.preventDefault();
          setDragging(true);
        }}
        onDragLeave={() => {
          setDragging(false);
        }}
        onDrop={(event) => {
          event.preventDefault();
          setDragging(false);
          queue.addFiles(event.dataTransfer.files);
        }}
        className={`glass-panel flex cursor-pointer flex-col items-center justify-center gap-3 rounded-zuno border-2 border-dashed p-8 text-center transition-colors focus-within:border-accent-cyan ${
          dragging ? 'border-accent-cyan bg-accent-blue/10' : 'border-line/80'
        }`}
      >
        <input
          id={inputId}
          type="file"
          multiple
          accept=".opus,.ogg,audio/ogg"
          aria-describedby={hintId}
          className="sr-only"
          onChange={(event) => {
            if (event.currentTarget.files) {
              queue.addFiles(event.currentTarget.files);
            }
            event.currentTarget.value = '';
          }}
        />
        <span className="text-base font-semibold text-copy">
          Arraste arquivos Opus ou selecione no dispositivo
        </span>
        <span id={hintId} className="max-w-prose text-sm text-copy-muted">
          Apenas .opus ou .ogg. Vários arquivos são enviados em fila, um por vez.
        </span>
        <span className="primary-button inline-flex min-h-11 items-center rounded-xl px-5 py-2 text-sm font-semibold">
          Selecionar arquivos
        </span>
      </label>

      {queue.rejected.length > 0 && (
        <div role="alert" className="rounded-zuno border border-red-500/40 bg-red-500/5 p-4">
          <p className="text-sm font-semibold text-copy">Arquivos não enviados</p>
          <ul className="mt-2 space-y-2">
            {queue.rejected.map((item) => (
              <li key={item.id} className="flex flex-wrap items-start justify-between gap-3">
                <div>
                  <p className="text-sm font-medium text-copy">{item.fileName}</p>
                  <p className="text-sm text-copy-muted">{item.reason}</p>
                </div>
                <button
                  type="button"
                  onClick={() => {
                    queue.dismissRejected(item.id);
                  }}
                  className="inline-flex min-h-9 items-center rounded-lg border border-line px-3 py-1.5 text-xs font-semibold text-copy hover:bg-surface-muted"
                >
                  Dispensar
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}

      {queue.items.length === 0 ? (
        <p className="text-sm text-copy-muted" role="status" aria-live="polite">
          Nenhum arquivo na fila.
        </p>
      ) : (
        <div>
          <div className="flex flex-wrap items-center justify-between gap-3">
            <h3 className="text-lg font-semibold text-copy">Fila de uploads</h3>
            {hasClearable && (
              <button
                type="button"
                onClick={queue.clearCompleted}
                className="inline-flex min-h-9 items-center rounded-lg border border-line px-3 py-1.5 text-xs font-semibold text-copy hover:bg-surface-muted"
              >
                Limpar concluídos
              </button>
            )}
          </div>

          <ul aria-live="polite" className="mt-3 space-y-3">
            {queue.items.map((item) => (
              <li key={item.id} className="rounded-zuno border border-line/60 bg-surface/50 p-4">
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="min-w-0">
                    <p className="truncate text-sm font-semibold text-copy">{item.fileName}</p>
                    <p className="text-xs text-copy-muted">{formatBytes(item.sizeBytes)}</p>
                  </div>
                  <p
                    className={`text-sm font-semibold ${statusTone(item.status)}`}
                    aria-live="polite"
                  >
                    {STATUS_LABELS[item.status]}
                    {item.status === 'sending' ? ` ${item.progress}%` : ''}
                  </p>
                </div>

                {(item.status === 'sending' || item.status === 'validating') && (
                  <div
                    role="progressbar"
                    aria-label={`Progresso de ${item.fileName}`}
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={item.progress}
                    className="mt-3 h-2 overflow-hidden rounded-full bg-surface-muted"
                  >
                    <div
                      aria-hidden="true"
                      className="h-full rounded-full bg-accent-blue transition-[width] motion-safe:duration-200"
                      style={{ width: `${item.progress}%` }}
                    />
                  </div>
                )}

                {item.status === 'error' && item.error && (
                  <p className="mt-3 text-sm text-red-200">{describeUploadError(item.error)}</p>
                )}

                {item.status === 'duplicate' && (
                  <div className="mt-3 space-y-2">
                    <p className="text-sm text-copy-muted">
                      Este arquivo já existe na biblioteca. Nada foi sobrescrito.
                    </p>
                    <a
                      href={routeHref({ name: 'library', kind: 'tracks' })}
                      className="inline-flex text-sm font-semibold text-accent-cyan underline underline-offset-4 hover:text-copy"
                    >
                      Já existe na biblioteca — ver faixa
                      {item.trackId ? ` (${item.trackId})` : ''}
                    </a>
                  </div>
                )}

                <div className="mt-3 flex flex-wrap gap-2">
                  {(item.status === 'queued' ||
                    item.status === 'sending' ||
                    item.status === 'validating') && (
                    <button
                      type="button"
                      onClick={() => {
                        queue.cancel(item.id);
                      }}
                      className="inline-flex min-h-9 items-center rounded-lg border border-line px-3 py-1.5 text-xs font-semibold text-copy hover:bg-surface-muted"
                    >
                      Cancelar
                    </button>
                  )}

                  {(item.status === 'error' || item.status === 'canceled') && (
                    <button
                      type="button"
                      onClick={() => {
                        queue.retry(item.id);
                      }}
                      className="primary-button inline-flex min-h-9 items-center rounded-lg px-3 py-1.5 text-xs font-semibold"
                    >
                      Tentar novamente
                    </button>
                  )}

                  {item.status === 'duplicate' && (
                    <button
                      type="button"
                      onClick={() => {
                        queue.discard(item.id);
                      }}
                      className="inline-flex min-h-9 items-center rounded-lg border border-line px-3 py-1.5 text-xs font-semibold text-copy hover:bg-surface-muted"
                    >
                      Descartar
                    </button>
                  )}

                  {item.status === 'error' && (
                    <button
                      type="button"
                      onClick={() => {
                        queue.discard(item.id);
                      }}
                      className="inline-flex min-h-9 items-center rounded-lg border border-line px-3 py-1.5 text-xs font-semibold text-copy hover:bg-surface-muted"
                    >
                      Remover
                    </button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}

      {hasIndexed && (
        <div
          role="status"
          className="flex flex-wrap items-center justify-between gap-3 rounded-zuno border border-accent-cyan/40 bg-accent-cyan/5 p-4"
        >
          <p className="text-sm text-copy">
            Upload concluído. A biblioteca já foi atualizada com as faixas indexadas.
          </p>
          <a
            href={routeHref({ name: 'library', kind: 'tracks' })}
            className="primary-button inline-flex min-h-10 items-center rounded-xl px-4 py-2 text-sm font-semibold"
          >
            Ir para a biblioteca
          </a>
        </div>
      )}
    </section>
  );
}
