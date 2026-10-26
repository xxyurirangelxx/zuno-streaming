import { useState } from 'react';
import { Artwork } from '../components/Artwork';
import {
  NextIcon,
  PauseIcon,
  PlayIcon,
  PreviousIcon,
  QueueIcon,
  RepeatIcon,
  RepeatOneIcon,
  ShuffleIcon,
  VolumeIcon,
  VolumeMuteIcon,
} from '../components/Icons';
import { formatClock, formatDurationMs } from '../lib/format';
import { usePlayer } from './PlayerProvider';
import type { RepeatMode } from './reducer';
import { QueuePanel } from './QueuePanel';

const REPEAT_LABEL: Record<RepeatMode, string> = {
  off: 'Repetir desativado',
  all: 'Repetir fila',
  one: 'Repetir faixa',
};

export function PlayerBar() {
  const { state, dispatch, currentTrack, seekTo, setVolume } = usePlayer();
  const [queueOpen, setQueueOpen] = useState(false);

  const hasTrack = currentTrack !== null;
  const durationMs = state.durationMs ?? 0;
  const maxSeek = Math.max(durationMs, 1);
  const artists = currentTrack?.artists.map((artist) => artist.name).join(', ') ?? '';

  return (
    <>
      <QueuePanel open={queueOpen} onClose={() => setQueueOpen(false)} />

      <footer
        aria-label="Player"
        className="glass-panel fixed inset-x-0 bottom-16 z-30 border-x-0 border-b-0 md:bottom-0"
      >
        <div className="mx-auto flex max-w-[1600px] flex-wrap items-center gap-x-3 gap-y-2 px-3 py-2 md:px-5 md:py-3">
          <div className="order-1 flex min-w-0 flex-1 basis-36 items-center gap-3">
            <Artwork
              artworkId={currentTrack?.artwork_id ?? null}
              alt={currentTrack ? `Capa de ${currentTrack.title}` : ''}
              className="h-11 w-11"
            />
            <div className="min-w-0">
              <p className="truncate text-sm font-semibold text-copy">
                {currentTrack?.title ?? 'Nenhuma faixa selecionada'}
              </p>
              <p className="truncate text-xs text-copy-muted">
                {currentTrack
                  ? artists || 'Artista desconhecido'
                  : 'Escolha uma faixa na biblioteca'}
              </p>
            </div>
          </div>

          <div className="order-2 flex items-center gap-1">
            <button
              type="button"
              onClick={() => dispatch({ type: 'previous' })}
              disabled={!hasTrack}
              aria-label="Faixa anterior"
              className="inline-flex h-10 w-10 items-center justify-center rounded-full text-copy hover:bg-surface-muted disabled:opacity-40"
            >
              <PreviousIcon className="h-5 w-5" />
            </button>
            <button
              type="button"
              onClick={() => dispatch({ type: 'toggle' })}
              disabled={!hasTrack}
              aria-label={state.isPlaying ? 'Pausar' : 'Reproduzir'}
              className="primary-button inline-flex h-11 w-11 items-center justify-center rounded-full disabled:cursor-not-allowed disabled:opacity-40"
            >
              {state.isPlaying ? (
                <PauseIcon className="h-5 w-5" />
              ) : (
                <PlayIcon className="h-5 w-5" />
              )}
            </button>
            <button
              type="button"
              onClick={() => dispatch({ type: 'next' })}
              disabled={!hasTrack}
              aria-label="Próxima faixa"
              className="inline-flex h-10 w-10 items-center justify-center rounded-full text-copy hover:bg-surface-muted disabled:opacity-40"
            >
              <NextIcon className="h-5 w-5" />
            </button>
          </div>

          <div className="order-4 flex w-full basis-full items-center gap-2 md:order-3 md:min-w-40 md:flex-1 md:basis-48">
            <span className="w-10 shrink-0 text-right font-mono text-[0.7rem] text-copy-muted">
              {formatDurationMs(state.currentTimeMs)}
            </span>
            <input
              type="range"
              min={0}
              max={maxSeek}
              step={1000}
              value={Math.min(state.currentTimeMs, maxSeek)}
              onChange={(event) => seekTo(Number(event.target.value))}
              disabled={!hasTrack || durationMs === 0}
              aria-label="Posição da faixa"
              aria-valuetext={`${formatClock(state.currentTimeMs / 1000)} de ${formatClock(durationMs / 1000)}`}
              className="h-1.5 w-full cursor-pointer accent-accent-blue disabled:cursor-not-allowed disabled:opacity-40"
            />
            <span className="w-10 shrink-0 font-mono text-[0.7rem] text-copy-muted">
              {formatDurationMs(durationMs || currentTrack?.duration_ms || null)}
            </span>
          </div>

          <div className="order-3 flex items-center gap-1 md:order-4">
            <button
              type="button"
              onClick={() => dispatch({ type: 'toggleShuffle' })}
              aria-pressed={state.shuffle}
              aria-label="Aleatório"
              className={`inline-flex h-9 w-9 items-center justify-center rounded-full hover:bg-surface-muted ${
                state.shuffle ? 'text-accent-blue' : 'text-copy-muted'
              }`}
            >
              <ShuffleIcon className="h-4 w-4" />
            </button>
            <button
              type="button"
              onClick={() => dispatch({ type: 'cycleRepeat' })}
              aria-label={REPEAT_LABEL[state.repeat]}
              title={REPEAT_LABEL[state.repeat]}
              className={`inline-flex h-9 w-9 items-center justify-center rounded-full hover:bg-surface-muted ${
                state.repeat === 'off' ? 'text-copy-muted' : 'text-accent-blue'
              }`}
            >
              {state.repeat === 'one' ? (
                <RepeatOneIcon className="h-4 w-4" />
              ) : (
                <RepeatIcon className="h-4 w-4" />
              )}
            </button>

            <button
              type="button"
              onClick={() => dispatch({ type: 'toggleMute' })}
              aria-label={state.muted ? 'Ativar som' : 'Silenciar'}
              className="inline-flex h-9 w-9 items-center justify-center rounded-full text-copy-muted hover:bg-surface-muted"
            >
              {state.muted || state.volume === 0 ? (
                <VolumeMuteIcon className="h-4 w-4" />
              ) : (
                <VolumeIcon className="h-4 w-4" />
              )}
            </button>
            <input
              type="range"
              min={0}
              max={100}
              step={5}
              value={Math.round((state.muted ? 0 : state.volume) * 100)}
              onChange={(event) => setVolume(Number(event.target.value) / 100)}
              aria-label="Volume"
              className="hidden h-1.5 w-20 cursor-pointer accent-accent-blue sm:block"
            />

            <button
              type="button"
              onClick={() => setQueueOpen((open) => !open)}
              aria-expanded={queueOpen}
              aria-label="Fila de reprodução"
              className={`inline-flex h-9 w-9 items-center justify-center rounded-full hover:bg-surface-muted ${
                queueOpen ? 'text-accent-blue' : 'text-copy-muted'
              }`}
            >
              <QueueIcon className="h-4 w-4" />
            </button>
          </div>

          {state.error && (
            <div
              role="alert"
              className="order-5 flex w-full basis-full items-center justify-between gap-3 rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-1.5 text-xs text-copy"
            >
              <span>{state.error}</span>
              <button
                type="button"
                onClick={() => dispatch({ type: 'clearError' })}
                className="shrink-0 font-semibold underline underline-offset-2"
              >
                Fechar aviso
              </button>
            </div>
          )}
        </div>
      </footer>
    </>
  );
}
