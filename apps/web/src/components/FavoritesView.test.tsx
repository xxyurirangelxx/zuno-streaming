import type { ZunoClient } from '@zuno/api-client';
import type { TrackSummary } from '@zuno/shared-types';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { AuthProvider } from '../app/AuthProvider';
import { FavoritesProvider } from '../favorites/FavoritesProvider';
import type { LibraryKind } from '../lib/router';
import { PlayerProvider } from '../player/PlayerProvider';
import { FavoritesView } from './FavoritesView';

const track: TrackSummary = {
  id: 't1',
  title: 'Faixa Favorita',
  duration_ms: 120_000,
  track_no: 1,
  disc_no: 1,
  album: { id: 'a1', title: 'Álbum' },
  artists: [{ id: 'ar1', name: 'Artista' }],
  artwork_id: null,
  available: true,
};

function createClient(favorites: { tracks: TrackSummary[]; albums: []; artists: [] }): ZunoClient {
  return {
    me: vi.fn().mockResolvedValue({ id: 'u1', username: 'ana', role: 'user' }),
    listFavorites: vi.fn().mockResolvedValue(favorites),
    artworkUrl: (id: string) => `/art/${id}`,
    streamUrl: (id: string) => `/stream/${id}`,
  } as unknown as ZunoClient;
}

function renderView(kind: LibraryKind, favorites: Parameters<typeof createClient>[0]) {
  const client = createClient(favorites);

  render(
    <AuthProvider client={client}>
      <FavoritesProvider client={client}>
        <PlayerProvider client={client}>
          <FavoritesView kind={kind} />
        </PlayerProvider>
      </FavoritesProvider>
    </AuthProvider>,
  );
}

describe('FavoritesView', () => {
  it('mostra estado vazio explicativo por aba', async () => {
    renderView('tracks', { tracks: [], albums: [], artists: [] });

    expect(await screen.findByText('Nenhuma faixa favorita')).toBeInTheDocument();
    expect(screen.getByText(/toque no coração ao lado de uma faixa/i)).toBeInTheDocument();
  });

  it('lista favoritos com coração preenchido', async () => {
    renderView('tracks', { tracks: [track], albums: [], artists: [] });

    expect(await screen.findByText('Faixa Favorita')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Remover Faixa Favorita dos favoritos' }),
    ).toHaveAttribute('aria-pressed', 'true');
  });
});
