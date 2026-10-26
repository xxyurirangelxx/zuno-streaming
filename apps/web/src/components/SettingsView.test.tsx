import { ZunoApiError } from '@zuno/api-client';
import type { ZunoClient } from '@zuno/api-client';
import type { UserPrivacy } from '@zuno/shared-types';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { AuthProvider } from '../app/AuthProvider';
import { ThemeProvider } from '../theme/ThemeProvider';
import { SettingsView } from './SettingsView';

interface Harness {
  client: ZunoClient;
  updatePreferences: ReturnType<typeof vi.fn>;
  updatePrivacy: ReturnType<typeof vi.fn>;
}

function createClient(
  privacy: UserPrivacy = { allow_cloud_history: false, history_retention_days: null },
): Harness {
  const updatePreferences = vi
    .fn()
    .mockImplementation((patch: { theme?: string; language?: string }) =>
      Promise.resolve({ theme: 'system', language: 'pt-BR', ...patch }),
    );
  const updatePrivacy = vi
    .fn()
    .mockImplementation((patch: Partial<UserPrivacy>) => Promise.resolve({ ...privacy, ...patch }));

  return {
    updatePreferences,
    updatePrivacy,
    client: {
      me: vi.fn().mockResolvedValue({ id: 'u1', username: 'ana', role: 'user' }),
      getPreferences: vi.fn().mockResolvedValue({ theme: 'system', language: 'pt-BR' }),
      updatePreferences,
      getPrivacy: vi.fn().mockResolvedValue(privacy),
      updatePrivacy,
    } as unknown as ZunoClient,
  };
}

function renderSettings(harness: Harness) {
  render(
    <AuthProvider client={harness.client}>
      <ThemeProvider client={harness.client}>
        <SettingsView />
      </ThemeProvider>
    </AuthProvider>,
  );
}

describe('SettingsView', () => {
  it('exibe o seletor de tema acessível e salva a escolha', async () => {
    const harness = createClient();
    renderSettings(harness);

    expect(await screen.findByRole('heading', { name: 'Configurações' })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /seguir o sistema/i })).toBeChecked();

    fireEvent.click(screen.getByRole('radio', { name: /^escuro/i }));

    await waitFor(() => {
      expect(harness.updatePreferences).toHaveBeenCalledWith({ theme: 'dark' });
    });
    expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
  });

  it('mostra pt-BR e mantém inglês desabilitado', async () => {
    renderSettings(createClient());

    expect(await screen.findByLabelText('Idioma')).toHaveValue('pt-BR');
    expect(screen.getByRole('option', { name: /português \(brasil\)/i })).toBeEnabled();
    expect(screen.getByRole('option', { name: /english/i })).toBeDisabled();
    expect(screen.getByText(/inglês está em breve/i)).toBeInTheDocument();
  });

  it('salva consentimento e retenção com atualização otimista', async () => {
    const harness = createClient();
    renderSettings(harness);

    const toggle = await screen.findByLabelText(/permitir enviar meu histórico/i);
    expect(toggle).not.toBeChecked();

    fireEvent.click(toggle);

    await waitFor(() => {
      expect(harness.updatePrivacy).toHaveBeenCalledWith({ allow_cloud_history: true });
    });
    expect(toggle).toBeChecked();

    fireEvent.change(screen.getByLabelText('Retenção do histórico'), {
      target: { value: '90' },
    });

    await waitFor(() => {
      expect(harness.updatePrivacy).toHaveBeenCalledWith({ history_retention_days: 90 });
    });
    expect(screen.getByLabelText('Retenção do histórico')).toHaveValue('90');
    expect(await screen.findByText('Histórico mantido por 90 dias.')).toBeInTheDocument();
  });

  it('reverte o consentimento e anuncia o erro quando o PUT falha', async () => {
    const harness = createClient();
    harness.updatePrivacy.mockRejectedValue(
      new ZunoApiError('network_error', 'Sem conexão.', null),
    );
    renderSettings(harness);

    const toggle = await screen.findByLabelText(/permitir enviar meu histórico/i);
    fireEvent.click(toggle);

    await waitFor(() => {
      expect(toggle).not.toBeChecked();
    });
    expect(await screen.findByText(/não foi possível conectar/i)).toBeInTheDocument();
  });
});
