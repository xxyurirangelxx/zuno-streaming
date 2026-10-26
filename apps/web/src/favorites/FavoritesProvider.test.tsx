import { ZunoApiError } from '@zuno/api-client';
import type { ZunoClient } from '@zuno/api-client';
import type { TrackSummary } from '@zuno/shared-types';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { FavoriteButton } from '../components/FavoriteButton';
import { FavoritesProvider } from './FavoritesProvider';

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

function client(overrides: Partial<ZunoClient>): ZunoClient {
  return {
    listFavorites: vi.fn().mockResolvedValue({ tracks: [track], albums: [], artists: [] }),
    setFavorite: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  } as unknown as ZunoClient;
}

function renderProvider(overrides: Partial<ZunoClient>) {
  render(
    <FavoritesProvider client={client(overrides)}>
      <FavoriteButton kind="track" entity={track} label={track.title} />
    </FavoritesProvider>,
  );
}

describe('FavoritesProvider', () => {
  it('faz atualização otimista e reverte quando a API falha', async () => {
    let rejectRequest!: (reason: unknown) => void;
    const setFavorite = vi.fn().mockReturnValue(
      new Promise<void>((_resolve, reject) => {
        rejectRequest = reject;
      }),
    );

    renderProvider({ setFavorite });

    const button = await screen.findByRole('button', {
      name: 'Remover Faixa dos favoritos',
    });
    expect(button).toHaveAttribute('aria-pressed', 'true');

    fireEvent.click(button);

    expect(button).toHaveAttribute('aria-pressed', 'false');
    expect(setFavorite).toHaveBeenCalledWith('track', 't1', false);

    await act(async () => {
      rejectRequest(new ZunoApiError('server_error', 'Falhou.', 500));
    });

    expect(button).toHaveAttribute('aria-pressed', 'true');
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'O servidor respondeu com HTTP 500. Tente novamente.',
    );
  });

  it('adiciona aos favoritos de forma otimista', async () => {
    const setFavorite = vi.fn().mockResolvedValue(undefined);

    renderProvider({
      listFavorites: vi.fn().mockResolvedValue({ tracks: [], albums: [], artists: [] }),
      setFavorite,
    });

    const button = await screen.findByRole('button', {
      name: 'Adicionar Faixa aos favoritos',
    });
    fireEvent.click(button);

    expect(button).toHaveAttribute('aria-pressed', 'true');
    expect(setFavorite).toHaveBeenCalledWith('track', 't1', true);
    expect(await screen.findByText('Adicionado aos favoritos.')).toBeInTheDocument();
  });
});
