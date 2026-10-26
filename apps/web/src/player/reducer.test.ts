import type { TrackSummary } from '@zuno/shared-types';
import { describe, expect, it, vi } from 'vitest';
import { initialPlayerState, playerReducer } from './reducer';
import type { PlayerState } from './reducer';

function track(id: string, durationMs = 180_000): TrackSummary {
  return {
    id,
    title: `Faixa ${id}`,
    duration_ms: durationMs,
    track_no: 1,
    disc_no: 1,
    album: { id: 'album-1', title: 'Álbum' },
    artists: [{ id: 'artist-1', name: 'Artista' }],
    artwork_id: null,
    available: true,
  };
}

const tracks = [track('a'), track('b'), track('c'), track('d')];

function playing(index = 0, queue = tracks): PlayerState {
  return playerReducer(initialPlayerState, { type: 'playQueue', tracks: queue, startIndex: index });
}

describe('playerReducer — fila e controles', () => {
  it('inicia a fila na faixa escolhida', () => {
    const state = playing(2);

    expect(state.currentIndex).toBe(2);
    expect(state.isPlaying).toBe(true);
    expect(state.queue).toHaveLength(4);
    expect(state.durationMs).toBe(180_000);
  });

  it('alterna play/pause', () => {
    const state = playing(0);
    const paused = playerReducer(state, { type: 'toggle' });

    expect(paused.isPlaying).toBe(false);
    expect(playerReducer(paused, { type: 'toggle' }).isPlaying).toBe(true);
  });

  it('avança e volta faixas, parando no fim com repeat off', () => {
    let state = playing(0);
    state = playerReducer(state, { type: 'next' });
    expect(state.currentIndex).toBe(1);

    state = playerReducer(state, { type: 'previous' });
    expect(state.currentIndex).toBe(0);

    state = playing(3);
    state = playerReducer(state, { type: 'next' });
    expect(state.isPlaying).toBe(false);
    expect(state.currentIndex).toBe(3);
  });

  it('repete a faixa atual ao terminar com repeat one', () => {
    let state = playing(1);
    state = playerReducer(state, { type: 'cycleRepeat' }); // all
    state = playerReducer(state, { type: 'cycleRepeat' }); // one
    state = { ...state, currentTimeMs: 180_000 };

    const next = playerReducer(state, { type: 'ended' });

    expect(next.repeat).toBe('one');
    expect(next.currentIndex).toBe(1);
    expect(next.currentTimeMs).toBe(0);
    expect(next.isPlaying).toBe(true);
  });

  it('reinicia a fila com repeat all ao terminar a última faixa', () => {
    let state = playing(3);
    state = playerReducer(state, { type: 'cycleRepeat' }); // all
    state = { ...state, currentTimeMs: 180_000 };

    const next = playerReducer(state, { type: 'ended' });

    expect(next.repeat).toBe('all');
    expect(next.currentIndex).toBe(0);
    expect(next.isPlaying).toBe(true);
  });

  it('cicla repeat off → all → one → off', () => {
    let state = initialPlayerState;
    state = playerReducer(state, { type: 'cycleRepeat' });
    expect(state.repeat).toBe('all');
    state = playerReducer(state, { type: 'cycleRepeat' });
    expect(state.repeat).toBe('one');
    state = playerReducer(state, { type: 'cycleRepeat' });
    expect(state.repeat).toBe('off');
  });

  it('faz shuffle sem repetir até esgotar a fila', () => {
    const random = vi.spyOn(Math, 'random').mockReturnValue(0);
    let state = playing(0);
    state = playerReducer(state, { type: 'toggleShuffle' });
    expect(state.shuffle).toBe(true);

    const visited = [state.currentIndex];

    for (let step = 0; step < 3; step += 1) {
      state = playerReducer(state, { type: 'next' });
      visited.push(state.currentIndex);
    }

    expect(new Set(visited).size).toBe(4);
    expect(visited).not.toContain(null);

    const afterPool = playerReducer(state, { type: 'next' });
    expect(afterPool.isPlaying).toBe(false);
    random.mockRestore();
  });

  it('recomeça o shuffle ao esgotar com repeat all, sem repetir o atual', () => {
    const random = vi.spyOn(Math, 'random').mockReturnValue(0);
    let state = playing(0);
    state = playerReducer(state, { type: 'toggleShuffle' });
    state = playerReducer(state, { type: 'cycleRepeat' }); // all

    for (let step = 0; step < 3; step += 1) {
      state = playerReducer(state, { type: 'next' });
    }

    expect(state.currentIndex).toBe(3);

    const restarted = playerReducer(state, { type: 'next' });
    expect(restarted.isPlaying).toBe(true);
    expect(restarted.currentIndex).not.toBe(3);
    random.mockRestore();
  });

  it('limita o seek entre 0 e a duração', () => {
    let state = playing(0);
    state = playerReducer(state, { type: 'setDuration', durationMs: 10_000 });

    expect(playerReducer(state, { type: 'seek', positionMs: 12_000 }).currentTimeMs).toBe(10_000);
    expect(playerReducer(state, { type: 'seek', positionMs: -50 }).currentTimeMs).toBe(0);
    expect(playerReducer(state, { type: 'seek', positionMs: 4_200 }).currentTimeMs).toBe(4_200);
  });

  it('remove e reordena sem corromper o índice atual', () => {
    let state = playing(1);

    state = playerReducer(state, { type: 'removeAt', index: 0 });
    expect(state.queue.map((item) => item.id)).toEqual(['b', 'c', 'd']);
    expect(state.currentIndex).toBe(0);

    state = playerReducer(state, { type: 'move', from: 0, to: 2 });
    expect(state.queue.map((item) => item.id)).toEqual(['c', 'd', 'b']);
    expect(state.currentIndex).toBe(2);

    state = playerReducer(state, { type: 'removeAt', index: 2 });
    expect(state.queue.map((item) => item.id)).toEqual(['c', 'd']);
    expect(state.currentIndex).toBe(1);
  });

  it('limpa a fila ao remover a última faixa', () => {
    let state = playing(0, [track('solo')]);
    state = playerReducer(state, { type: 'removeAt', index: 0 });

    expect(state.queue).toEqual([]);
    expect(state.currentIndex).toBeNull();
    expect(state.isPlaying).toBe(false);
  });

  it('registra erro de reprodução sem quebrar o estado', () => {
    const state = playerReducer(playing(0), {
      type: 'playbackError',
      message: 'Falhou',
    });

    expect(state.error).toBe('Falhou');
    expect(state.isPlaying).toBe(false);
    expect(playerReducer(state, { type: 'clearError' }).error).toBeNull();
  });

  it('limita volume entre 0 e 1', () => {
    let state = playing(0);
    state = playerReducer(state, { type: 'setVolume', volume: 2 });
    expect(state.volume).toBe(1);
    state = playerReducer(state, { type: 'setVolume', volume: Number.NaN });
    expect(state.volume).toBe(0);
  });
});
