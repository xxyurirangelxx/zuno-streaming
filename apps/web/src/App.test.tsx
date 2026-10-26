import { ZunoApiError } from '@zuno/api-client';
import type { ZunoClient } from '@zuno/api-client';
import type { User } from '@zuno/shared-types';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import App from './App';

const user: User = { id: 'u1', username: 'ana', role: 'user' };
const admin: User = { id: 'u0', username: 'admin', role: 'admin' };

interface MockClient {
  client: ZunoClient;
  me: ReturnType<typeof vi.fn>;
  setupStatus: ReturnType<typeof vi.fn>;
  setupClaim: ReturnType<typeof vi.fn>;
  login: ReturnType<typeof vi.fn>;
  logout: ReturnType<typeof vi.fn>;
  listTracks: ReturnType<typeof vi.fn>;
}

function createMockClient(): MockClient {
  const me = vi.fn();
  const setupStatus = vi.fn();
  const setupClaim = vi.fn();
  const login = vi.fn();
  const logout = vi.fn().mockResolvedValue(undefined);
  const listTracks = vi.fn().mockResolvedValue({ items: [], next_cursor: null });

  const client = {
    health: vi.fn(),
    me,
    setupStatus,
    setupClaim,
    login,
    logout,
    refresh: vi.fn(),
    redeemInvitation: vi.fn(),
    createInvitation: vi.fn(),
    listInvitations: vi.fn(),
    revokeInvitation: vi.fn(),
    listTracks,
    listAlbums: vi.fn().mockResolvedValue({ items: [], next_cursor: null }),
    listArtists: vi.fn().mockResolvedValue({ items: [], next_cursor: null }),
    search: vi.fn().mockResolvedValue({ query: '', tracks: [], albums: [], artists: [] }),
    getTrack: vi.fn(),
    listPlaylists: vi.fn().mockResolvedValue([]),
    createPlaylist: vi.fn(),
    getPlaylist: vi.fn(),
    updatePlaylist: vi.fn(),
    deletePlaylist: vi.fn(),
    addPlaylistItem: vi.fn(),
    removePlaylistItem: vi.fn(),
    movePlaylistItem: vi.fn(),
    listFavorites: vi.fn().mockResolvedValue({ tracks: [], albums: [], artists: [] }),
    setFavorite: vi.fn(),
    addFavorite: vi.fn(),
    removeFavorite: vi.fn(),
    artworkUrl: (id: string, size = 'thumb') => `/api/v1/artworks/${id}?size=${size}`,
    streamUrl: (id: string) => `/api/v1/tracks/${id}/stream`,
  } as unknown as ZunoClient;

  return { client, me, setupStatus, setupClaim, login, logout, listTracks };
}

describe('fluxo de sessão', () => {
  beforeEach(() => {
    window.location.hash = '';
  });

  it('mostra setup quando a instância exige configuração e entra após reivindicar', async () => {
    const mock = createMockClient();
    mock.me.mockRejectedValue(new ZunoApiError('unauthorized', 'Sem sessão.', 401));
    mock.setupStatus.mockResolvedValue({ required: true });
    mock.setupClaim.mockResolvedValue({ user: admin });

    render(<App client={mock.client} />);

    expect(
      await screen.findByRole('heading', { name: 'Configurar instância' }),
    ).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Código de setup'), {
      target: { value: 'ABC12345' },
    });
    fireEvent.change(screen.getByLabelText('Usuário'), { target: { value: 'admin' } });
    fireEvent.change(screen.getByLabelText('Senha'), { target: { value: 'segredo123' } });
    fireEvent.click(screen.getByRole('button', { name: 'Criar administrador' }));

    expect(await screen.findByText('Olá, admin')).toBeInTheDocument();
    expect(mock.setupClaim).toHaveBeenCalledWith({
      code: 'ABC12345',
      username: 'admin',
      password: 'segredo123',
    });
  });

  it('mostra login quando o setup já foi feito e entra na biblioteca', async () => {
    const mock = createMockClient();
    mock.me.mockRejectedValue(new ZunoApiError('unauthorized', 'Sem sessão.', 401));
    mock.setupStatus.mockResolvedValue({ required: false });
    mock.login.mockResolvedValue({ user });

    render(<App client={mock.client} />);

    expect(await screen.findByRole('heading', { name: 'Entrar' })).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Usuário'), { target: { value: 'ana' } });
    fireEvent.change(screen.getByLabelText('Senha'), { target: { value: 'segredo123' } });
    fireEvent.click(screen.getByRole('button', { name: 'Entrar' }));

    expect(await screen.findByText('Olá, ana')).toBeInTheDocument();
    expect(mock.login).toHaveBeenCalledWith({ username: 'ana', password: 'segredo123' });

    expect(screen.getAllByRole('link', { name: 'Biblioteca' }).length).toBeGreaterThan(0);
    await waitFor(() => {
      expect(mock.listTracks).toHaveBeenCalled();
    });
  });

  it('retorna ao login quando uma chamada autenticada recebe 401', async () => {
    const mock = createMockClient();
    mock.me.mockResolvedValue(user);
    mock.listTracks.mockRejectedValue(new ZunoApiError('unauthorized', 'Sessão expirada.', 401));

    render(<App client={mock.client} />);

    expect(await screen.findByText('Olá, ana')).toBeInTheDocument();
    expect(await screen.findByRole('heading', { name: 'Entrar' })).toBeInTheDocument();
  });

  it('encerra a sessão pelo menu do usuário', async () => {
    const mock = createMockClient();
    mock.me.mockResolvedValue(user);

    render(<App client={mock.client} />);

    expect(await screen.findByText('Olá, ana')).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole('button', { name: 'Sair' })[0] as HTMLElement);

    await waitFor(() => {
      expect(mock.logout).toHaveBeenCalled();
    });
    expect(await screen.findByRole('heading', { name: 'Entrar' })).toBeInTheDocument();
  });

  it('exibe erro acionável quando o servidor não responde', async () => {
    const mock = createMockClient();
    mock.me.mockRejectedValue(new ZunoApiError('network_error', 'Sem conexão.', null));

    render(<App client={mock.client} />);

    expect(await screen.findByText('Não foi possível conectar ao Zuno')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Tentar novamente' })).toBeInTheDocument();
  });
});
