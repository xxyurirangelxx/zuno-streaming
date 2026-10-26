import type { ZunoClient } from '@zuno/api-client';
import type { TrackSummary, User } from '@zuno/shared-types';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { AuthProvider } from '../app/AuthProvider';
import { PlayerProvider } from '../player/PlayerProvider';
import { LibraryView } from './LibraryView';

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

function createClient(listTracks: ReturnType<typeof vi.fn>): ZunoClient {
  return {
    me: vi.fn().mockResolvedValue(user),
    listTracks,
    listAlbums: vi.fn().mockResolvedValue({ items: [], next_cursor: null }),
    listArtists: vi.fn().mockResolvedValue({ items: [], next_cursor: null }),
    artworkUrl: (id: string) => `/art/${id}`,
    streamUrl: (id: string) => `/stream/${id}`,
  } as unknown as ZunoClient;
}

function Providers({ client, children }: { client: ZunoClient; children: ReactNode }) {
  return (
    <AuthProvider client={client}>
      <PlayerProvider client={client}>{children}</PlayerProvider>
    </AuthProvider>
  );
}

describe('LibraryView', () => {
  it('mostra skeleton e depois as faixas', async () => {
    let resolvePage!: (value: { items: TrackSummary[]; next_cursor: null }) => void;
    const listTracks = vi.fn().mockReturnValue(
      new Promise((resolve) => {
        resolvePage = resolve;
      }),
    );

    render(
      <Providers client={createClient(listTracks)}>
        <LibraryView kind="tracks" />
      </Providers>,
    );

    expect(screen.getByText('Carregando todas as faixas…')).toBeInTheDocument();

    resolvePage({ items: [track('t1', 'Primeira')], next_cursor: null });

    expect(await screen.findByText('Primeira')).toBeInTheDocument();
  });

  it('mostra estado vazio acionável', async () => {
    const listTracks = vi.fn().mockResolvedValue({ items: [], next_cursor: null });

    render(
      <Providers client={createClient(listTracks)}>
        <LibraryView kind="tracks" />
      </Providers>,
    );

    expect(await screen.findByText('Biblioteca vazia')).toBeInTheDocument();
    expect(screen.getByText(/envie arquivos opus/i)).toBeInTheDocument();
  });

  it('mostra erro e permite tentar novamente', async () => {
    const listTracks = vi.fn().mockRejectedValue(new Error('falhou'));

    render(
      <Providers client={createClient(listTracks)}>
        <LibraryView kind="tracks" />
      </Providers>,
    );

    expect(await screen.findByText('Não foi possível carregar')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Tentar novamente' }));

    await waitFor(() => {
      expect(listTracks).toHaveBeenCalledTimes(2);
    });
  });

  it('carrega mais páginas com next_cursor', async () => {
    const listTracks = vi
      .fn()
      .mockResolvedValueOnce({ items: [track('t1', 'Primeira')], next_cursor: 'c2' })
      .mockResolvedValueOnce({ items: [track('t2', 'Segunda')], next_cursor: null });

    render(
      <Providers client={createClient(listTracks)}>
        <LibraryView kind="tracks" />
      </Providers>,
    );

    expect(await screen.findByText('Primeira')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Carregar mais' }));

    expect(await screen.findByText('Segunda')).toBeInTheDocument();
    expect(listTracks).toHaveBeenLastCalledWith({ limit: 50, cursor: 'c2' }, expect.anything());
    expect(screen.queryByRole('button', { name: 'Carregar mais' })).not.toBeInTheDocument();
  });
});
