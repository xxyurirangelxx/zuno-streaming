import type { ZunoClient } from '@zuno/api-client';
import type { SearchResponse, User } from '@zuno/shared-types';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AuthProvider } from '../app/AuthProvider';
import { PlayerProvider } from '../player/PlayerProvider';
import { SearchView } from './SearchView';

const user: User = { id: 'u1', username: 'ana', role: 'user' };

const response: SearchResponse = {
  query: 'mpb',
  tracks: [
    {
      id: 't1',
      title: 'Faixa Encontrada',
      duration_ms: 61_000,
      track_no: 1,
      disc_no: 1,
      album: { id: 'a1', title: 'Álbum' },
      artists: [{ id: 'ar1', name: 'Artista' }],
      artwork_id: null,
      available: true,
    },
  ],
  albums: [],
  artists: [],
};

function createClient(search: ReturnType<typeof vi.fn>): ZunoClient {
  return {
    me: vi.fn().mockResolvedValue(user),
    search,
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

describe('SearchView', () => {
  beforeEach(() => {
    vi.useRealTimers();
  });

  it('faz a busca após o debounce de 300 ms', async () => {
    const search = vi.fn().mockResolvedValue(response);
    render(
      <Providers client={createClient(search)}>
        <SearchView />
      </Providers>,
    );

    const input = screen.getByRole('searchbox', { name: /buscar faixas/i });
    fireEvent.change(input, { target: { value: 'mpb' } });

    expect(search).not.toHaveBeenCalled();

    await waitFor(() => {
      expect(search).toHaveBeenCalledWith('mpb', expect.anything());
    });

    expect(await screen.findByText('Faixa Encontrada')).toBeInTheDocument();
  });

  it('não busca termos vazios e limpa os resultados', async () => {
    const search = vi.fn().mockResolvedValue(response);
    render(
      <Providers client={createClient(search)}>
        <SearchView />
      </Providers>,
    );

    const input = screen.getByRole('searchbox', { name: /buscar faixas/i });
    fireEvent.change(input, { target: { value: '   ' } });

    await new Promise((resolve) => setTimeout(resolve, 350));
    expect(search).not.toHaveBeenCalled();

    fireEvent.change(input, { target: { value: 'mpb' } });
    expect(await screen.findByText('Faixa Encontrada')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Limpar busca' }));

    expect(screen.queryByText('Faixa Encontrada')).not.toBeInTheDocument();
    expect(screen.getByText('Comece digitando')).toBeInTheDocument();
  });

  it('mostra estado vazio quando não há resultados', async () => {
    const search = vi.fn().mockResolvedValue({
      query: 'zzz',
      tracks: [],
      albums: [],
      artists: [],
    });
    render(
      <Providers client={createClient(search)}>
        <SearchView />
      </Providers>,
    );

    fireEvent.change(screen.getByRole('searchbox', { name: /buscar faixas/i }), {
      target: { value: 'zzz' },
    });

    expect(await screen.findByText('Nenhum resultado')).toBeInTheDocument();
  });

  it('mostra erro acionável', async () => {
    const search = vi.fn().mockRejectedValue(new Error('falhou'));
    render(
      <Providers client={createClient(search)}>
        <SearchView />
      </Providers>,
    );

    fireEvent.change(screen.getByRole('searchbox', { name: /buscar faixas/i }), {
      target: { value: 'mpb' },
    });

    expect(await screen.findByText('Não foi possível carregar')).toBeInTheDocument();
  });
});
