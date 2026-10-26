import { ZunoApiError } from '@zuno/api-client';
import type { ZunoClient } from '@zuno/api-client';
import type { PlaylistSummary, User } from '@zuno/shared-types';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { AuthProvider } from '../app/AuthProvider';
import { PlaylistsView } from './PlaylistsView';

const user: User = { id: 'u1', username: 'ana', role: 'user' };

function playlist(overrides: Partial<PlaylistSummary> = {}): PlaylistSummary {
  return {
    id: 'p1',
    name: 'Favoritas',
    visibility: 'private',
    track_count: 3,
    updated_at: '2026-10-26T10:00:00Z',
    ...overrides,
  };
}

function createClient(overrides: Partial<ZunoClient>): ZunoClient {
  return {
    me: vi.fn().mockResolvedValue(user),
    listPlaylists: vi.fn().mockResolvedValue([playlist()]),
    ...overrides,
  } as unknown as ZunoClient;
}

describe('PlaylistsView', () => {
  it('cria, renomeia e exclui playlists com confirmação explícita', async () => {
    const listPlaylists = vi.fn().mockResolvedValue([playlist()]);
    const createPlaylist = vi
      .fn()
      .mockResolvedValue(playlist({ id: 'p2', name: 'Viagem', track_count: 0 }));
    const updatePlaylist = vi.fn().mockResolvedValue(playlist({ name: 'Favoritas 2026' }));
    const deletePlaylist = vi.fn().mockResolvedValue(undefined);

    render(
      <AuthProvider
        client={createClient({ listPlaylists, createPlaylist, updatePlaylist, deletePlaylist })}
      >
        <PlaylistsView />
      </AuthProvider>,
    );

    expect(await screen.findByText('Favoritas')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Nova playlist' }));
    fireEvent.change(screen.getByLabelText('Nome da playlist'), {
      target: { value: 'Viagem' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Criar' }));

    expect(await screen.findByText('Viagem')).toBeInTheDocument();
    expect(createPlaylist).toHaveBeenCalledWith({ name: 'Viagem' });

    fireEvent.click(screen.getByRole('button', { name: 'Renomear Favoritas' }));
    fireEvent.change(screen.getByLabelText('Nome da playlist'), {
      target: { value: 'Favoritas 2026' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Salvar' }));

    expect(await screen.findByText('Favoritas 2026')).toBeInTheDocument();
    expect(updatePlaylist).toHaveBeenCalledWith('p1', { name: 'Favoritas 2026' });

    fireEvent.click(screen.getByRole('button', { name: 'Excluir Viagem' }));

    expect(deletePlaylist).not.toHaveBeenCalled();
    expect(screen.getByText(/será removida permanentemente/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Excluir playlist' }));

    await waitFor(() => {
      expect(screen.queryByText('Viagem')).not.toBeInTheDocument();
    });
    expect(deletePlaylist).toHaveBeenCalledWith('p2');
  });

  it('valida nome vazio e feedback de duplicada', async () => {
    const createPlaylist = vi
      .fn()
      .mockRejectedValue(new ZunoApiError('duplicate', 'Nome já usado.', 409));

    render(
      <AuthProvider
        client={createClient({ listPlaylists: vi.fn().mockResolvedValue([]), createPlaylist })}
      >
        <PlaylistsView />
      </AuthProvider>,
    );

    expect(await screen.findByText('Nenhuma playlist ainda')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Criar playlist' }));
    fireEvent.click(screen.getByRole('button', { name: 'Criar' }));
    expect(screen.getByText('Informe um nome para a playlist.')).toBeInTheDocument();
    expect(createPlaylist).not.toHaveBeenCalled();

    fireEvent.change(screen.getByLabelText('Nome da playlist'), {
      target: { value: 'Favoritas' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Criar' }));

    expect(await screen.findByText('Já existe uma playlist com esse nome.')).toBeInTheDocument();
  });
});
