import type { ZunoClient } from '@zuno/api-client';
import type { HistoryItem } from '@zuno/shared-types';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { AuthProvider } from '../app/AuthProvider';
import { HistoryView } from './HistoryView';

function item(id: string, title: string, overrides: Partial<HistoryItem> = {}): HistoryItem {
  return {
    id,
    track: {
      id: `t-${id}`,
      title,
      duration_ms: 120_000,
      track_no: 1,
      disc_no: 1,
      album: { id: 'a1', title: 'Álbum' },
      artists: [{ id: 'ar1', name: 'Artista' }],
      artwork_id: null,
      available: true,
    },
    event_kind: 'play',
    seconds: null,
    occurred_at: '2026-10-26T10:00:00Z',
    ...overrides,
  };
}

interface Harness {
  client: ZunoClient;
  listHistory: ReturnType<typeof vi.fn>;
  clearHistory: ReturnType<typeof vi.fn>;
}

function createClient(overrides: Partial<ZunoClient> = {}): Harness {
  const listHistory = vi.fn().mockResolvedValue({ items: [], next_cursor: null });
  const clearHistory = vi.fn().mockResolvedValue(undefined);

  return {
    listHistory,
    clearHistory,
    client: {
      me: vi.fn().mockResolvedValue({ id: 'u1', username: 'ana', role: 'user' }),
      listHistory,
      clearHistory,
      artworkUrl: (id: string) => `/art/${id}`,
      streamUrl: (id: string) => `/stream/${id}`,
      ...overrides,
    } as unknown as ZunoClient,
  };
}

function renderHistory(harness: Harness) {
  render(
    <AuthProvider client={harness.client}>
      <HistoryView />
    </AuthProvider>,
  );
}

describe('HistoryView', () => {
  it('lista eventos com faixa, artista, tipo e horário', async () => {
    const harness = createClient();
    harness.listHistory.mockResolvedValue({
      items: [
        item('h1', 'Primeira', { event_kind: 'play' }),
        item('h2', 'Segunda', { event_kind: 'complete', seconds: 120 }),
      ],
      next_cursor: null,
    });

    renderHistory(harness);

    expect(await screen.findByText('Primeira')).toBeInTheDocument();
    expect(screen.getByText('Segunda')).toBeInTheDocument();
    expect(screen.getAllByText('Artista')).toHaveLength(2);
    expect(screen.getByText('Reprodução')).toBeInTheDocument();
    expect(screen.getByText('Concluída')).toBeInTheDocument();
    expect(screen.getByText('2:00')).toBeInTheDocument();
  });

  it('pagina com cursor e carrega a próxima página', async () => {
    const harness = createClient();
    harness.listHistory
      .mockResolvedValueOnce({ items: [item('h1', 'Primeira')], next_cursor: 'c2' })
      .mockResolvedValueOnce({ items: [item('h2', 'Segunda')], next_cursor: null });

    renderHistory(harness);

    expect(await screen.findByText('Primeira')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Carregar mais' }));

    expect(await screen.findByText('Segunda')).toBeInTheDocument();
    expect(harness.listHistory).toHaveBeenLastCalledWith(
      { limit: 30, cursor: 'c2' },
      expect.anything(),
    );
    expect(screen.queryByRole('button', { name: 'Carregar mais' })).not.toBeInTheDocument();
  });

  it('limpa o histórico apenas após confirmação explícita', async () => {
    const harness = createClient();
    harness.listHistory
      .mockResolvedValueOnce({ items: [item('h1', 'Primeira')], next_cursor: null })
      .mockResolvedValueOnce({ items: [], next_cursor: null });

    renderHistory(harness);

    expect(await screen.findByText('Primeira')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Limpar histórico' }));

    expect(screen.getByRole('dialog', { name: 'Limpar todo o histórico?' })).toBeInTheDocument();
    expect(harness.clearHistory).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Sim, limpar histórico' }));

    await waitFor(() => {
      expect(harness.clearHistory).toHaveBeenCalledTimes(1);
    });
    expect(await screen.findByText('Seu histórico está vazio')).toBeInTheDocument();
  });

  it('mostra estado vazio explicativo', async () => {
    renderHistory(createClient());

    expect(await screen.findByText('Seu histórico está vazio')).toBeInTheDocument();
    expect(screen.getByText(/o histórico é individual, privado/i)).toBeInTheDocument();
  });
});
