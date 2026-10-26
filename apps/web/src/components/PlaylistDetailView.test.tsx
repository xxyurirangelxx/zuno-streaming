import type { ZunoClient } from '@zuno/api-client';
import type { PlaylistDetail, TrackSummary, User } from '@zuno/shared-types';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { AuthProvider } from '../app/AuthProvider';
import { PlayerBar } from '../player/PlayerBar';
import { PlayerProvider } from '../player/PlayerProvider';
import { PlaylistDetailView } from './PlaylistDetailView';

const user: User = { id: 'u1', username: 'ana', role: 'user' };

function track(id: string, title: string): TrackSummary {
  return {
    id,
    title,
    duration_ms: 120_000,
    track_no: 1,
    disc_no: 1,
    album: { id: 'a1', title: 'Álbum' },
    artists: [{ id: 'ar1', name: 'Artista' }],
    artwork_id: null,
    available: true,
  };
}

const detail: PlaylistDetail = {
  id: 'p1',
  name: 'Favoritas',
  visibility: 'private',
  track_count: 2,
  updated_at: '2026-10-26T10:00:00Z',
  items: [track('t1', 'Primeira'), track('t2', 'Segunda')],
};

function createClient(overrides: Partial<ZunoClient>): ZunoClient {
  return {
    me: vi.fn().mockResolvedValue(user),
    getPlaylist: vi.fn().mockResolvedValue(detail),
    listPlaylists: vi.fn().mockResolvedValue([]),
    listFavorites: vi.fn().mockResolvedValue({ tracks: [], albums: [], artists: [] }),
    artworkUrl: (id: string) => `/art/${id}`,
    streamUrl: (id: string) => `/stream/${id}`,
    ...overrides,
  } as unknown as ZunoClient;
}

function renderView(overrides: Partial<ZunoClient>) {
  const client = createClient(overrides);

  render(
    <AuthProvider client={client}>
      <PlayerProvider client={client}>
        <PlaylistDetailView id="p1" />
        <PlayerBar />
      </PlayerProvider>
    </AuthProvider>,
  );
}

describe('PlaylistDetailView', () => {
  it('move faixas para cima e para baixo pela posição', async () => {
    const movePlaylistItem = vi.fn().mockResolvedValue(undefined);

    renderView({ movePlaylistItem });

    expect(await screen.findByText('Primeira')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Mover Primeira para baixo' }));

    await waitFor(() => {
      expect(movePlaylistItem).toHaveBeenCalledWith('p1', 't1', { position: 1 });
    });
    expect(await screen.findByText('“Primeira” movida para a posição 2.')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Mover Primeira para cima' }));

    await waitFor(() => {
      expect(movePlaylistItem).toHaveBeenCalledWith('p1', 't1', { position: 0 });
    });
  });

  it('remove faixas e mostra o estado vazio', async () => {
    const removePlaylistItem = vi.fn().mockResolvedValue(undefined);

    renderView({
      getPlaylist: vi
        .fn()
        .mockResolvedValue({ ...detail, items: [detail.items[0]], track_count: 1 }),
      removePlaylistItem,
    });

    expect(await screen.findByText('Primeira')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Remover Primeira da playlist' }));

    expect(await screen.findByText('Playlist vazia')).toBeInTheDocument();
    expect(removePlaylistItem).toHaveBeenCalledWith('p1', 't1');
  });

  it('toca tudo carregando a fila do player existente', async () => {
    renderView({});

    expect(await screen.findByText('Primeira')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Tocar tudo' }));

    expect(await screen.findByRole('button', { name: 'Pausar' })).toBeInTheDocument();
  });

  it('mostra estado vazio explicativo', async () => {
    renderView({
      getPlaylist: vi.fn().mockResolvedValue({ ...detail, items: [], track_count: 0 }),
    });

    expect(
      await screen.findByText(
        'Playlist vazia — adicione faixas pelo botão de playlist em qualquer música da biblioteca.',
      ),
    ).toBeInTheDocument();
  });
});
