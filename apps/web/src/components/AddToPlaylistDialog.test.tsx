import { ZunoApiError } from '@zuno/api-client';
import type { ZunoClient } from '@zuno/api-client';
import type { PlaylistSummary, TrackSummary, User } from '@zuno/shared-types';
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { AuthProvider } from '../app/AuthProvider';
import { AddToPlaylistDialog } from './AddToPlaylistDialog';

const user: User = { id: 'u1', username: 'ana', role: 'user' };

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

const playlist: PlaylistSummary = {
  id: 'p1',
  name: 'Favoritas',
  visibility: 'private',
  track_count: 2,
  updated_at: '2026-10-26T10:00:00Z',
};

function createClient(overrides: Partial<ZunoClient>): ZunoClient {
  return {
    me: vi.fn().mockResolvedValue(user),
    listPlaylists: vi.fn().mockResolvedValue([playlist]),
    ...overrides,
  } as unknown as ZunoClient;
}

function renderDialog(client: ZunoClient) {
  return render(
    <AuthProvider client={client}>
      <AddToPlaylistDialog track={track} onClose={() => undefined} />
    </AuthProvider>,
  );
}

describe('AddToPlaylistDialog', () => {
  it('adiciona a faixa a uma playlist existente', async () => {
    const addPlaylistItem = vi.fn().mockResolvedValue({ position: 2 });

    renderDialog(createClient({ addPlaylistItem }));

    fireEvent.click(await screen.findByRole('button', { name: /Favoritas/ }));

    expect(await screen.findByText(/foi adicionada a “Favoritas”/)).toBeInTheDocument();
    expect(addPlaylistItem).toHaveBeenCalledWith('p1', { track_id: 't1' });
    expect(screen.getByText('3 faixas')).toBeInTheDocument();
  });

  it('mostra feedback específico quando a faixa já está na playlist', async () => {
    const addPlaylistItem = vi
      .fn()
      .mockRejectedValue(new ZunoApiError('duplicate', 'Faixa já presente.', 409));

    renderDialog(createClient({ addPlaylistItem }));

    fireEvent.click(await screen.findByRole('button', { name: /Favoritas/ }));

    expect(await screen.findByText('Esta faixa já está na playlist.')).toBeInTheDocument();
  });

  it('cria uma nova playlist e adiciona a faixa', async () => {
    const createPlaylist = vi.fn().mockResolvedValue({
      ...playlist,
      id: 'p2',
      name: 'Viagem',
      track_count: 0,
    });
    const addPlaylistItem = vi.fn().mockResolvedValue({ position: 0 });

    renderDialog(
      createClient({
        listPlaylists: vi.fn().mockResolvedValue([]),
        createPlaylist,
        addPlaylistItem,
      }),
    );

    expect(await screen.findByText(/Nenhuma playlist ainda/i)).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Criar nova playlist'), {
      target: { value: 'Viagem' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Criar' }));

    expect(await screen.findByText(/foi adicionada a “Viagem”/)).toBeInTheDocument();
    expect(createPlaylist).toHaveBeenCalledWith({ name: 'Viagem' });
    expect(addPlaylistItem).toHaveBeenCalledWith('p2', { track_id: 't1' });
  });
});
