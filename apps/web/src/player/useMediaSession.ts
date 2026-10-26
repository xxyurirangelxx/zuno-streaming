import type { TrackSummary } from '@zuno/shared-types';
import { useEffect } from 'react';

export interface MediaSessionHandlers {
  play: () => void;
  pause: () => void;
  next: () => void;
  previous: () => void;
  seekTo: (positionMs: number) => void;
}

export interface MediaSessionOptions {
  track: TrackSummary | null;
  artworkUrl: string | null;
  durationMs: number | null;
  positionMs: number;
  handlers: MediaSessionHandlers;
}

function getMediaSession(): MediaSession | null {
  if (typeof navigator === 'undefined' || !('mediaSession' in navigator)) {
    return null;
  }

  return navigator.mediaSession;
}

/** Metadados e handlers; falhas são silenciosas (navegador sem suporte). */
export function useMediaSession({
  track,
  artworkUrl,
  durationMs,
  positionMs,
  handlers,
}: MediaSessionOptions): void {
  useEffect(() => {
    const session = getMediaSession();

    if (!session) {
      return;
    }

    try {
      if (track && typeof MediaMetadata !== 'undefined') {
        session.metadata = new MediaMetadata({
          title: track.title,
          artist: track.artists.map((artist) => artist.name).join(', '),
          album: track.album?.title ?? '',
          artwork: artworkUrl ? [{ src: artworkUrl, sizes: '640x640' }] : [],
        });
      } else {
        session.metadata = null;
      }

      session.setActionHandler('play', handlers.play);
      session.setActionHandler('pause', handlers.pause);
      session.setActionHandler('nexttrack', handlers.next);
      session.setActionHandler('previoustrack', handlers.previous);
      session.setActionHandler('seekto', (details) => {
        if (typeof details.seekTime === 'number') {
          handlers.seekTo(details.seekTime * 1000);
        }
      });
    } catch {
      // API experimental ou parcialmente suportada.
    }

    return () => {
      try {
        session.setActionHandler('play', null);
        session.setActionHandler('pause', null);
        session.setActionHandler('nexttrack', null);
        session.setActionHandler('previoustrack', null);
        session.setActionHandler('seekto', null);
      } catch {
        // Ignora navegadores com implementação incompleta.
      }
    };
  }, [track, artworkUrl, handlers]);

  useEffect(() => {
    const session = getMediaSession();

    if (!session || !track || durationMs === null || durationMs <= 0) {
      return;
    }

    try {
      session.setPositionState({
        duration: durationMs / 1000,
        position: Math.min(Math.max(0, positionMs / 1000), durationMs / 1000),
        playbackRate: 1,
      });
    } catch {
      // Sem suporte a position state: ignora.
    }
  }, [track, durationMs, positionMs]);
}
