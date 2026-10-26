import { ZunoApiError } from '@zuno/api-client';
import type { ZunoClient } from '@zuno/api-client';
import type { ThemePreference, UserPreferences } from '@zuno/shared-types';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { applyTheme, ThemeProvider, usePreferences } from './ThemeProvider';

function createClient(preferences: UserPreferences): {
  client: ZunoClient;
  getPreferences: ReturnType<typeof vi.fn>;
  updatePreferences: ReturnType<typeof vi.fn>;
} {
  const getPreferences = vi.fn().mockResolvedValue(preferences);
  const updatePreferences = vi.fn();

  return {
    client: { getPreferences, updatePreferences } as unknown as ZunoClient,
    getPreferences,
    updatePreferences,
  };
}

function Probe() {
  const { preferences, setTheme } = usePreferences();

  return (
    <div>
      <p>{preferences.theme}</p>
      {(['light', 'dark', 'system'] satisfies ThemePreference[]).map((theme) => (
        <button
          key={theme}
          type="button"
          onClick={() => {
            void setTheme(theme);
          }}
        >
          {theme}
        </button>
      ))}
    </div>
  );
}

describe('tema', () => {
  afterEach(() => {
    document.documentElement.removeAttribute('data-theme');
  });

  it('aplica e remove data-theme no documento', () => {
    applyTheme('dark');
    expect(document.documentElement).toHaveAttribute('data-theme', 'dark');

    applyTheme('light');
    expect(document.documentElement).toHaveAttribute('data-theme', 'light');

    applyTheme('system');
    expect(document.documentElement).not.toHaveAttribute('data-theme');
  });

  it('carrega a preferência do servidor antes de renderizar o app', async () => {
    const { client } = createClient({ theme: 'dark', language: 'pt-BR' });

    render(
      <ThemeProvider client={client}>
        <Probe />
      </ThemeProvider>,
    );

    expect(await screen.findByText('dark', { selector: 'p' })).toBeInTheDocument();
    expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
  });

  it('faz atualização otimista e reverte quando o PUT falha', async () => {
    const { client, updatePreferences } = createClient({ theme: 'dark', language: 'pt-BR' });
    updatePreferences.mockRejectedValue(new ZunoApiError('network_error', 'Sem conexão.', null));

    render(
      <ThemeProvider client={client}>
        <Probe />
      </ThemeProvider>,
    );

    expect(await screen.findByText('dark', { selector: 'p' })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'light' }));

    // Otimista: o atributo muda antes da resposta do servidor.
    expect(document.documentElement).toHaveAttribute('data-theme', 'light');
    expect(updatePreferences).toHaveBeenCalledWith({ theme: 'light' });

    await waitFor(() => {
      expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
    });
    expect(await screen.findByText(/não foi possível conectar/i)).toBeInTheDocument();
  });

  it('segue o sistema removendo o atributo ao salvar system', async () => {
    const { client, updatePreferences } = createClient({ theme: 'dark', language: 'pt-BR' });
    updatePreferences.mockResolvedValue({ theme: 'system', language: 'pt-BR' });

    render(
      <ThemeProvider client={client}>
        <Probe />
      </ThemeProvider>,
    );

    expect(await screen.findByText('dark', { selector: 'p' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'system' }));

    await waitFor(() => {
      expect(document.documentElement).not.toHaveAttribute('data-theme');
    });
    expect(updatePreferences).toHaveBeenCalledWith({ theme: 'system' });
  });

  it('cai para o tema do sistema quando o carregamento falha', async () => {
    const getPreferences = vi
      .fn()
      .mockRejectedValue(new ZunoApiError('network_error', 'Erro.', null));
    const client = { getPreferences } as unknown as ZunoClient;

    render(
      <ThemeProvider client={client}>
        <Probe />
      </ThemeProvider>,
    );

    expect(await screen.findByText('system', { selector: 'p' })).toBeInTheDocument();
    expect(document.documentElement).not.toHaveAttribute('data-theme');
  });
});
