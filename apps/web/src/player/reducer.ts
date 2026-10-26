import type { TrackSummary } from '@zuno/shared-types';

export type RepeatMode = 'off' | 'one' | 'all';

export interface PlayerState {
  queue: TrackSummary[];
  currentIndex: number | null;
  isPlaying: boolean;
  /** Incrementa a cada (re)início de faixa, inclusive repeat one. */
  playbackToken: number;
  currentTimeMs: number;
  durationMs: number | null;
  volume: number;
  muted: boolean;
  shuffle: boolean;
  repeat: RepeatMode;
  /** Histórico de índices tocados em shuffle, para não repetir antes de esgotar. */
  shuffleHistory: number[];
  error: string | null;
}

export type PlayerAction =
  | { type: 'playQueue'; tracks: TrackSummary[]; startIndex?: number }
  | { type: 'playIndex'; index: number }
  | { type: 'toggle' }
  | { type: 'setPlaying'; value: boolean }
  | { type: 'next' }
  | { type: 'previous' }
  | { type: 'ended' }
  | { type: 'seek'; positionMs: number }
  | { type: 'setCurrentTime'; positionMs: number }
  | { type: 'setDuration'; durationMs: number | null }
  | { type: 'setVolume'; volume: number }
  | { type: 'toggleMute' }
  | { type: 'toggleShuffle' }
  | { type: 'cycleRepeat' }
  | { type: 'removeAt'; index: number }
  | { type: 'move'; from: number; to: number }
  | { type: 'playbackError'; message: string }
  | { type: 'clearError' };

export const initialPlayerState: PlayerState = {
  queue: [],
  currentIndex: null,
  isPlaying: false,
  playbackToken: 0,
  currentTimeMs: 0,
  durationMs: null,
  volume: 1,
  muted: false,
  shuffle: false,
  repeat: 'off',
  shuffleHistory: [],
  error: null,
};

const REPEAT_ORDER: RepeatMode[] = ['off', 'all', 'one'];

export function clampVolume(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }

  return Math.min(1, Math.max(0, value));
}

export function clampSeek(positionMs: number, durationMs: number | null): number {
  if (!Number.isFinite(positionMs) || positionMs < 0) {
    return 0;
  }

  if (durationMs !== null && Number.isFinite(durationMs) && durationMs > 0) {
    return Math.min(positionMs, durationMs);
  }

  return positionMs;
}

function nextRepeat(mode: RepeatMode): RepeatMode {
  const position = REPEAT_ORDER.indexOf(mode);
  return REPEAT_ORDER[(position + 1) % REPEAT_ORDER.length] ?? 'off';
}

interface ShuffleStep {
  index: number | null;
  history: number[];
}

function stepShuffle(state: PlayerState, allowRestart: boolean, random: () => number): ShuffleStep {
  const { queue, currentIndex, shuffleHistory } = state;

  if (queue.length === 0) {
    return { index: null, history: shuffleHistory };
  }

  const seen = new Set(shuffleHistory);
  let baseHistory = shuffleHistory;
  let candidates = queue
    .map((_, index) => index)
    .filter((index) => index !== currentIndex && !seen.has(index));

  if (candidates.length === 0 && allowRestart) {
    // Ciclo esgotado: recomeça o shuffle sem repetir a faixa atual.
    baseHistory = currentIndex === null ? [] : [currentIndex];
    candidates = queue.map((_, index) => index).filter((index) => index !== currentIndex);

    if (candidates.length === 0 && queue.length === 1) {
      candidates = [0];
    }
  }

  if (candidates.length === 0) {
    return { index: null, history: shuffleHistory };
  }

  const position = Math.min(candidates.length - 1, Math.floor(random() * candidates.length));
  const chosen = candidates[position];

  if (chosen === undefined) {
    return { index: null, history: shuffleHistory };
  }

  return { index: chosen, history: [...baseHistory, chosen] };
}

function trackDuration(track: TrackSummary | undefined): number | null {
  if (!track || track.duration_ms === null) {
    return null;
  }

  return track.duration_ms;
}

function withCurrent(state: PlayerState, index: number | null, isPlaying: boolean): PlayerState {
  const track = index === null ? undefined : state.queue[index];

  return {
    ...state,
    currentIndex: index,
    isPlaying,
    playbackToken: state.playbackToken + 1,
    currentTimeMs: 0,
    durationMs: trackDuration(track),
    error: null,
  };
}

interface NextOptions {
  auto: boolean;
  random: () => number;
}

function computeNext(state: PlayerState, options: NextOptions): PlayerState {
  const { queue, currentIndex, repeat, shuffle } = state;

  if (queue.length === 0) {
    return state;
  }

  if (options.auto && repeat === 'one' && currentIndex !== null) {
    return {
      ...state,
      currentTimeMs: 0,
      isPlaying: true,
      playbackToken: state.playbackToken + 1,
    };
  }

  const allowRestart = repeat === 'all';

  if (shuffle) {
    const step = stepShuffle(state, allowRestart, options.random);

    if (step.index === null) {
      return { ...state, isPlaying: false };
    }

    return withCurrent({ ...state, shuffleHistory: step.history }, step.index, true);
  }

  if (currentIndex === null) {
    return withCurrent(state, 0, true);
  }

  const next = currentIndex + 1;

  if (next < queue.length) {
    return withCurrent(state, next, true);
  }

  if (allowRestart) {
    return withCurrent(state, 0, true);
  }

  return { ...state, isPlaying: false };
}

function computePrevious(state: PlayerState): PlayerState {
  const { queue, currentIndex, shuffle, shuffleHistory } = state;

  if (queue.length === 0 || currentIndex === null) {
    return state;
  }

  if (shuffle) {
    const history = shuffleHistory.slice(0, -1);
    const previous = history[history.length - 1];

    if (previous !== undefined) {
      return withCurrent({ ...state, shuffleHistory: history }, previous, true);
    }
  }

  const previous = Math.max(0, currentIndex - 1);
  return withCurrent(
    { ...state, shuffleHistory: shuffle ? shuffleHistory.slice(0, -1) : shuffleHistory },
    previous,
    true,
  );
}

function removeAt(state: PlayerState, index: number): PlayerState {
  if (index < 0 || index >= state.queue.length) {
    return state;
  }

  const queue = state.queue.filter((_, position) => position !== index);
  const history = state.shuffleHistory
    .filter((position) => position !== index)
    .map((position) => (position > index ? position - 1 : position));

  if (queue.length === 0) {
    return {
      ...state,
      queue,
      currentIndex: null,
      isPlaying: false,
      currentTimeMs: 0,
      durationMs: null,
      shuffleHistory: [],
    };
  }

  if (state.currentIndex === null) {
    return { ...state, queue, shuffleHistory: history };
  }

  if (index < state.currentIndex) {
    return {
      ...state,
      queue,
      currentIndex: state.currentIndex - 1,
      shuffleHistory: history,
    };
  }

  if (index > state.currentIndex) {
    return { ...state, queue, shuffleHistory: history };
  }

  const nextIndex = Math.min(state.currentIndex, queue.length - 1);
  return withCurrent({ ...state, queue, shuffleHistory: history }, nextIndex, state.isPlaying);
}

function move(state: PlayerState, from: number, to: number): PlayerState {
  if (from === to || from < 0 || from >= state.queue.length) {
    return state;
  }

  const target = Math.min(state.queue.length - 1, Math.max(0, to));

  if (target === from) {
    return state;
  }

  const queue = [...state.queue];
  const [moved] = queue.splice(from, 1);

  if (!moved) {
    return state;
  }

  queue.splice(target, 0, moved);

  let currentIndex = state.currentIndex;

  if (currentIndex !== null) {
    if (currentIndex === from) {
      currentIndex = target;
    } else if (from < currentIndex && target >= currentIndex) {
      currentIndex -= 1;
    } else if (from > currentIndex && target <= currentIndex) {
      currentIndex += 1;
    }
  }

  return { ...state, queue, currentIndex };
}

export function playerReducer(state: PlayerState, action: PlayerAction): PlayerState {
  switch (action.type) {
    case 'playQueue': {
      if (action.tracks.length === 0) {
        return state;
      }

      const startIndex = Math.min(action.tracks.length - 1, Math.max(0, action.startIndex ?? 0));

      return {
        ...state,
        queue: action.tracks,
        currentIndex: startIndex,
        isPlaying: true,
        playbackToken: state.playbackToken + 1,
        currentTimeMs: 0,
        durationMs: trackDuration(action.tracks[startIndex]),
        error: null,
        shuffleHistory: state.shuffle ? [startIndex] : [],
      };
    }

    case 'playIndex': {
      if (action.index < 0 || action.index >= state.queue.length) {
        return state;
      }

      const shuffleHistory = state.shuffle
        ? [...state.shuffleHistory.filter((index) => index !== action.index), action.index]
        : state.shuffleHistory;

      return withCurrent({ ...state, shuffleHistory }, action.index, true);
    }

    case 'toggle':
      if (state.currentIndex === null) {
        return state;
      }
      return { ...state, isPlaying: !state.isPlaying, error: null };

    case 'setPlaying':
      return { ...state, isPlaying: action.value };

    case 'next':
      return computeNext(state, { auto: false, random: Math.random });

    case 'previous':
      return computePrevious(state);

    case 'ended':
      return computeNext(state, { auto: true, random: Math.random });

    case 'seek':
      return { ...state, currentTimeMs: clampSeek(action.positionMs, state.durationMs) };

    case 'setCurrentTime':
      return { ...state, currentTimeMs: clampSeek(action.positionMs, state.durationMs) };

    case 'setDuration':
      return {
        ...state,
        durationMs:
          action.durationMs !== null && Number.isFinite(action.durationMs) && action.durationMs > 0
            ? action.durationMs
            : null,
      };

    case 'setVolume':
      return { ...state, volume: clampVolume(action.volume), muted: false };

    case 'toggleMute':
      return { ...state, muted: !state.muted };

    case 'toggleShuffle':
      return {
        ...state,
        shuffle: !state.shuffle,
        shuffleHistory: !state.shuffle && state.currentIndex !== null ? [state.currentIndex] : [],
      };

    case 'cycleRepeat':
      return { ...state, repeat: nextRepeat(state.repeat) };

    case 'removeAt':
      return removeAt(state, action.index);

    case 'move':
      return move(state, action.from, action.to);

    case 'playbackError':
      return { ...state, isPlaying: false, error: action.message };

    case 'clearError':
      return { ...state, error: null };

    default:
      return state;
  }
}
