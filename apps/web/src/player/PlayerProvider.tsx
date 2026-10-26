import type { ZunoClient } from '@zuno/api-client';
import type { TrackSummary } from '@zuno/shared-types';
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
} from 'react';
import type { Dispatch, ReactNode } from 'react';
import { clampSeek, initialPlayerState, playerReducer } from './reducer';
import type { PlayerAction, PlayerState } from './reducer';
import { useMediaSession } from './useMediaSession';

export interface PlayerContextValue {
  state: PlayerState;
  dispatch: Dispatch<PlayerAction>;
  currentTrack: TrackSummary | null;
  seekTo: (positionMs: number) => void;
  setVolume: (volume: number) => void;
}

const PlayerContext = createContext<PlayerContextValue | null>(null);

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  const tag = target.tagName.toLowerCase();

  return (
    tag === 'input' ||
    tag === 'textarea' ||
    tag === 'select' ||
    target.isContentEditable ||
    target.getAttribute('role') === 'textbox'
  );
}

export function PlayerProvider({ client, children }: { client: ZunoClient; children: ReactNode }) {
  const [state, dispatch] = useReducer(playerReducer, initialPlayerState);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const appliedTokenRef = useRef(0);
  const currentTrack =
    state.currentIndex === null ? null : (state.queue[state.currentIndex] ?? null);

  const seekTo = useCallback(
    (positionMs: number) => {
      const clamped = clampSeek(positionMs, state.durationMs);
      const audio = audioRef.current;

      if (audio) {
        try {
          audio.currentTime = clamped / 1000;
        } catch {
          // Mídia ainda sem metadados: o reducer mantém a posição na UI.
        }
      }

      dispatch({ type: 'seek', positionMs: clamped });
    },
    [state.durationMs],
  );

  const setVolume = useCallback((volume: number) => {
    dispatch({ type: 'setVolume', volume });
  }, []);

  // Origem do stream: mesma origem via proxy, autenticada pelo cookie de sessão.
  useEffect(() => {
    const audio = audioRef.current;

    if (!audio) {
      return;
    }

    if (!currentTrack) {
      audio.removeAttribute('src');
      audio.load();
      return;
    }

    const nextSrc = client.streamUrl(currentTrack.id);

    if (audio.getAttribute('src') !== nextSrc) {
      audio.src = nextSrc;
      audio.load();
    }
  }, [client, currentTrack]);

  useEffect(() => {
    const audio = audioRef.current;

    if (!audio || !currentTrack) {
      return;
    }

    if (state.isPlaying) {
      if (appliedTokenRef.current !== state.playbackToken) {
        appliedTokenRef.current = state.playbackToken;

        try {
          audio.currentTime = 0;
        } catch {
          // Metadados ainda não carregados: o navegador inicia do começo.
        }
      }

      void audio.play().catch(() => {
        dispatch({
          type: 'playbackError',
          message: 'Não foi possível reproduzir esta faixa. Verifique o arquivo e tente novamente.',
        });
      });
    } else {
      audio.pause();
    }
  }, [state.isPlaying, state.playbackToken, currentTrack]);

  useEffect(() => {
    const audio = audioRef.current;

    if (audio) {
      audio.volume = state.muted ? 0 : state.volume;
    }
  }, [state.volume, state.muted]);

  useEffect(() => {
    const audio = audioRef.current;

    if (!audio) {
      return;
    }

    const onTimeUpdate = () => {
      dispatch({ type: 'setCurrentTime', positionMs: audio.currentTime * 1000 });
    };
    const onDurationChange = () => {
      dispatch({
        type: 'setDuration',
        durationMs: Number.isFinite(audio.duration) ? audio.duration * 1000 : null,
      });
    };
    const onEnded = () => {
      dispatch({ type: 'ended' });
    };
    const onError = () => {
      dispatch({
        type: 'playbackError',
        message: 'Falha ao carregar o áudio. A faixa pode estar indisponível no servidor.',
      });
    };

    audio.addEventListener('timeupdate', onTimeUpdate);
    audio.addEventListener('loadedmetadata', onDurationChange);
    audio.addEventListener('durationchange', onDurationChange);
    audio.addEventListener('ended', onEnded);
    audio.addEventListener('error', onError);

    return () => {
      audio.removeEventListener('timeupdate', onTimeUpdate);
      audio.removeEventListener('loadedmetadata', onDurationChange);
      audio.removeEventListener('durationchange', onDurationChange);
      audio.removeEventListener('ended', onEnded);
      audio.removeEventListener('error', onError);
    };
  }, []);

  // Atalhos: Espaço, ←/→ (5 s), Shift+←/→ (faixa).
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) {
        return;
      }

      if (isTypingTarget(event.target)) {
        return;
      }

      if (state.currentIndex === null) {
        return;
      }

      switch (event.key) {
        case ' ':
        case 'Spacebar':
          event.preventDefault();
          dispatch({ type: 'toggle' });
          break;
        case 'ArrowRight':
          event.preventDefault();
          if (event.shiftKey) {
            dispatch({ type: 'next' });
          } else {
            seekTo(state.currentTimeMs + 5000);
          }
          break;
        case 'ArrowLeft':
          event.preventDefault();
          if (event.shiftKey) {
            dispatch({ type: 'previous' });
          } else {
            seekTo(state.currentTimeMs - 5000);
          }
          break;
        default:
          break;
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
    };
  }, [seekTo, state.currentIndex, state.currentTimeMs]);

  const mediaHandlers = useMemo(
    () => ({
      play: () => dispatch({ type: 'setPlaying', value: true }),
      pause: () => dispatch({ type: 'setPlaying', value: false }),
      next: () => dispatch({ type: 'next' }),
      previous: () => dispatch({ type: 'previous' }),
      seekTo,
    }),
    [seekTo],
  );

  useMediaSession({
    track: currentTrack,
    artworkUrl:
      currentTrack && currentTrack.artwork_id !== null
        ? client.artworkUrl(currentTrack.artwork_id, 'card')
        : null,
    durationMs: state.durationMs,
    positionMs: state.currentTimeMs,
    handlers: mediaHandlers,
  });

  const value = useMemo<PlayerContextValue>(
    () => ({ state, dispatch, currentTrack, seekTo, setVolume }),
    [state, currentTrack, seekTo, setVolume],
  );

  return (
    <PlayerContext.Provider value={value}>
      {children}
      <audio ref={audioRef} preload="metadata" className="hidden" />
    </PlayerContext.Provider>
  );
}

export function usePlayer(): PlayerContextValue {
  const context = useContext(PlayerContext);

  if (!context) {
    throw new Error('usePlayer precisa estar dentro de PlayerProvider.');
  }

  return context;
}
