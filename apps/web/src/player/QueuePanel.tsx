import { formatDurationMs } from '../lib/format';
import { ChevronDownIcon, ChevronUpIcon, CloseIcon, TrashIcon } from '../components/Icons';
import { usePlayer } from './PlayerProvider';

interface QueuePanelProps {
  open: boolean;
  onClose: () => void;
}

export function QueuePanel({ open, onClose }: QueuePanelProps) {
  const { state, dispatch } = usePlayer();

  if (!open) {
    return null;
  }

  return (
    <aside
      aria-label="Fila de reprodução"
      className="glass-panel fixed inset-x-3 bottom-52 z-40 flex max-h-[60vh] flex-col overflow-hidden rounded-zuno md:inset-x-auto md:right-4 md:bottom-28 md:w-96"
    >
      <div className="flex items-center justify-between gap-3 border-b border-line/60 px-4 py-3">
        <div>
          <p className="text-sm font-semibold text-copy">Fila de reprodução</p>
          <p className="text-xs text-copy-muted">
            {state.queue.length} {state.queue.length === 1 ? 'faixa' : 'faixas'}
          </p>
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label="Fechar fila"
          className="inline-flex h-9 w-9 items-center justify-center rounded-full text-copy-muted hover:bg-surface-muted hover:text-copy"
        >
          <CloseIcon className="h-4 w-4" />
        </button>
      </div>

      {state.queue.length === 0 ? (
        <p className="px-4 py-6 text-sm text-copy-muted">
          A fila está vazia. Toque uma faixa na biblioteca para começar.
        </p>
      ) : (
        <ol className="min-h-0 flex-1 divide-y divide-line/50 overflow-y-auto px-2 py-1">
          {state.queue.map((track, index) => {
            const isCurrent = state.currentIndex === index;

            return (
              <li
                key={`${track.id}-${index}`}
                className={`flex items-center gap-2 rounded-lg px-2 py-2 ${
                  isCurrent ? 'bg-accent-blue/10' : ''
                }`}
              >
                <button
                  type="button"
                  onClick={() => dispatch({ type: 'playIndex', index })}
                  aria-current={isCurrent ? 'true' : undefined}
                  aria-label={`Tocar ${track.title}`}
                  className="min-w-0 flex-1 text-left"
                >
                  <span
                    className={`block truncate text-sm ${isCurrent ? 'font-semibold text-accent-blue' : 'text-copy'}`}
                  >
                    {track.title}
                  </span>
                  <span className="block truncate text-xs text-copy-muted">
                    {track.artists.map((artist) => artist.name).join(', ') ||
                      'Artista desconhecido'}{' '}
                    · {formatDurationMs(track.duration_ms)}
                  </span>
                </button>

                <div className="flex shrink-0 items-center">
                  <button
                    type="button"
                    disabled={index === 0}
                    onClick={() => dispatch({ type: 'move', from: index, to: index - 1 })}
                    aria-label={`Mover ${track.title} para cima`}
                    className="inline-flex h-8 w-8 items-center justify-center rounded-full text-copy-muted hover:bg-surface-muted hover:text-copy disabled:opacity-30"
                  >
                    <ChevronUpIcon className="h-4 w-4" />
                  </button>
                  <button
                    type="button"
                    disabled={index === state.queue.length - 1}
                    onClick={() => dispatch({ type: 'move', from: index, to: index + 1 })}
                    aria-label={`Mover ${track.title} para baixo`}
                    className="inline-flex h-8 w-8 items-center justify-center rounded-full text-copy-muted hover:bg-surface-muted hover:text-copy disabled:opacity-30"
                  >
                    <ChevronDownIcon className="h-4 w-4" />
                  </button>
                  <button
                    type="button"
                    onClick={() => dispatch({ type: 'removeAt', index })}
                    aria-label={`Remover ${track.title} da fila`}
                    className="inline-flex h-8 w-8 items-center justify-center rounded-full text-copy-muted hover:bg-surface-muted hover:text-copy"
                  >
                    <TrashIcon className="h-4 w-4" />
                  </button>
                </div>
              </li>
            );
          })}
        </ol>
      )}
    </aside>
  );
}
