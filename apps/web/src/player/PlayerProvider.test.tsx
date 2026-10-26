import { ZunoApiError } from '@zuno/api-client';
import type { ZunoClient } from '@zuno/api-client';
import type { TrackSummary } from '@zuno/shared-types';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { AuthProvider, useSession } from '../app/AuthProvider';
import { PlayerProvider, usePlayer } from './PlayerProvider';

const track: TrackSummary = {
  id: 't1',
  title: 'Faixa',
  duration_ms: 120_000,
  track_no: 1,
  disc_no: 1,
  album: { id: 'a1', title: 'Álbum' },
  artists: [{ id: 'ar1', name: 'Artista' }],
  artwork_id: null,
  available: true,
};

function Harness() {
  const session = useSession();
  const { dispatch } = usePlayer();

  return (
    <div>
      <p>estado: {session.state.kind}</p>
      <button type="button" onClick={() => dispatch({ type: 'playQueue', tracks: [track] })}>
        tocar
      </button>
      <button type="button" onClick={() => dispatch({ type: 'toggle' })}>
        alternar
      </button>
    </div>
  );
}

function createClient(recordHistoryEvent: ReturnType<typeof vi.fn>): ZunoClient {
  return {
    me: vi.fn().mockResolvedValue({ id: 'u1', username: 'ana', role: 'user' }),
    recordHistoryEvent,
    artworkUrl: (id: string) => `/art/${id}`,
    streamUrl: (id: string) => `/stream/${id}`,
  } as unknown as ZunoClient;
}

function renderPlayer(recordHistoryEvent: ReturnType<typeof vi.fn>) {
  const client = createClient(recordHistoryEvent);

  render(
    <AuthProvider client={client}>
      <PlayerProvider client={client}>
        <Harness />
      </PlayerProvider>
    </AuthProvider>,
  );
}

describe('PlayerProvider e histórico', () => {
  it('registra play uma vez por faixa e complete ao terminar', async () => {
    const recordHistoryEvent = vi.fn().mockResolvedValue(undefined);
    renderPlayer(recordHistoryEvent);

    expect(await screen.findByText('estado: authenticated')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'tocar' }));

    await waitFor(() => {
      expect(recordHistoryEvent).toHaveBeenCalledWith({ track_id: 't1', event_kind: 'play' });
    });

    // Pausa e retomada não geram novo play.
    fireEvent.click(screen.getByRole('button', { name: 'alternar' }));
    fireEvent.click(screen.getByRole('button', { name: 'alternar' }));

    const playCalls = recordHistoryEvent.mock.calls.filter(
      ([payload]) => (payload as { event_kind: string }).event_kind === 'play',
    );
    expect(playCalls).toHaveLength(1);

    const audio = document.querySelector('audio');
    expect(audio).not.toBeNull();
    fireEvent(audio as HTMLAudioElement, new Event('ended'));

    await waitFor(() => {
      expect(recordHistoryEvent).toHaveBeenCalledWith({
        track_id: 't1',
        event_kind: 'complete',
        seconds: 120,
      });
    });
  });

  it('silencia falhas de registro sem quebrar o player', async () => {
    const recordHistoryEvent = vi
      .fn()
      .mockRejectedValue(new ZunoApiError('network_error', 'Sem conexão.', null));
    renderPlayer(recordHistoryEvent);

    expect(await screen.findByText('estado: authenticated')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'tocar' }));

    await waitFor(() => {
      expect(recordHistoryEvent).toHaveBeenCalled();
    });
    expect(screen.getByRole('button', { name: 'alternar' })).toBeEnabled();
  });
});
