import type { TrackSummary } from '@zuno/shared-types';
import { useState } from 'react';
import type { ReactNode } from 'react';
import { formatDurationMs } from '../lib/format';
import { usePlayer } from '../player/PlayerProvider';
import { AddToPlaylistDialog } from './AddToPlaylistDialog';
import { Artwork } from './Artwork';
import { FavoriteButton } from './FavoriteButton';
import { PauseIcon, PlayIcon, PlaylistIcon } from './Icons';

interface TrackListProps {
  tracks: TrackSummary[];
  emptyMessage?: string;
  showAlbum?: boolean;
  actions?: (track: TrackSummary, index: number) => ReactNode;
}

export function TrackList({
  tracks,
  emptyMessage = 'Nenhuma faixa encontrada.',
  showAlbum = true,
  actions,
}: TrackListProps) {
  const { state, dispatch } = usePlayer();
  const [playlistTrack, setPlaylistTrack] = useState<TrackSummary | null>(null);

  if (tracks.length === 0) {
    return <p className="px-1 py-6 text-sm text-copy-muted">{emptyMessage}</p>;
  }

  return (
    <ol className="divide-y divide-line/50">
      {tracks.map((track, index) => {
        const isCurrent =
          state.currentIndex !== null && state.queue[state.currentIndex]?.id === track.id;
        const isPlaying = isCurrent && state.isPlaying;
        const artists =
          track.artists.map((artist) => artist.name).join(', ') || 'Artista desconhecido';

        return (
          <li
            key={track.id}
            className={`flex items-center gap-3 rounded-lg px-2 py-2.5 transition-colors motion-safe:duration-150 ${
              isCurrent ? 'bg-accent-blue/10' : 'hover:bg-surface-muted/70'
            }`}
          >
            <button
              type="button"
              disabled={!track.available}
              onClick={() => {
                if (isCurrent) {
                  dispatch({ type: 'toggle' });
                } else {
                  dispatch({ type: 'playQueue', tracks, startIndex: index });
                }
              }}
              aria-label={`Tocar ${track.title}`}
              className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-full border border-line/70 text-copy transition-colors hover:border-accent-blue hover:text-accent-blue disabled:cursor-not-allowed disabled:opacity-40"
            >
              {isPlaying ? <PauseIcon className="h-4 w-4" /> : <PlayIcon className="h-4 w-4" />}
            </button>

            <Artwork artworkId={track.artwork_id} alt="" className="h-11 w-11" />

            <div className="min-w-0 flex-1">
              <p
                className={`truncate text-sm font-semibold ${isCurrent ? 'text-accent-blue' : 'text-copy'}`}
                aria-current={isCurrent ? 'true' : undefined}
              >
                {track.title}
              </p>
              <p className="truncate text-xs text-copy-muted">
                {artists}
                {!track.available && (
                  <span className="ml-2 rounded-full border border-line px-2 py-0.5 text-[0.65rem] font-semibold">
                    Indisponível
                  </span>
                )}
              </p>
            </div>

            {showAlbum && (
              <p className="hidden w-48 truncate text-xs text-copy-muted md:block">
                {track.album?.title ?? '—'}
              </p>
            )}

            <span className="w-12 shrink-0 text-right font-mono text-xs text-copy-muted">
              {formatDurationMs(track.duration_ms)}
            </span>

            {actions?.(track, index)}

            <FavoriteButton kind="track" entity={track} label={track.title} />

            <button
              type="button"
              onClick={() => {
                setPlaylistTrack(track);
              }}
              aria-label={`Adicionar ${track.title} à playlist`}
              title="Adicionar à playlist"
              className="inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-full text-copy-muted transition-colors hover:bg-surface-muted hover:text-copy"
            >
              <PlaylistIcon className="h-4 w-4" />
            </button>
          </li>
        );
      })}

      {playlistTrack && (
        <AddToPlaylistDialog track={playlistTrack} onClose={() => setPlaylistTrack(null)} />
      )}
    </ol>
  );
}
