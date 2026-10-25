import type { HealthResponse } from '@zuno/shared-types';
import type * as ZunoApiClientModule from '@zuno/api-client';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { healthMock } = vi.hoisted(() => ({
  healthMock: vi.fn<() => Promise<HealthResponse>>(),
}));

vi.mock('@zuno/api-client', async (importOriginal) => {
  const actual = await importOriginal<typeof ZunoApiClientModule>();
  return { ...actual, createZunoClient: () => ({ health: healthMock }) };
});

import { ZunoApiError } from '@zuno/api-client';
import App from './App';

const healthyResponse: HealthResponse = {
  status: 'ok',
  service: 'zuno-server',
  version: '0.1.0',
  uptime_seconds: 125,
  database: { status: 'ok' },
};

const degradedResponse: HealthResponse = {
  ...healthyResponse,
  status: 'degraded',
  database: { status: 'error' },
};

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });

  return { promise, resolve, reject };
}

describe('App — conectividade M0', () => {
  beforeEach(() => {
    healthMock.mockReset();
  });

  it('mostra loading e depois o estado operacional com versão e uptime', async () => {
    const deferred = createDeferred<HealthResponse>();
    healthMock.mockReturnValue(deferred.promise);

    render(<App />);

    expect(screen.getByText('Consultando a saúde do servidor…')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Verificando…' })).toBeDisabled();

    await act(async () => {
      deferred.resolve(healthyResponse);
    });

    expect(
      await screen.findByText('Servidor operacional', undefined, { timeout: 3000 }),
    ).toBeInTheDocument();
    expect(screen.getByText('zuno-server')).toBeInTheDocument();
    expect(screen.getByText('0.1.0')).toBeInTheDocument();
    expect(screen.getByText('2 min 5 s')).toBeInTheDocument();
    expect(screen.getByText('Operacional')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Verificar novamente' })).toBeEnabled();
  });

  it('exibe estado degradado para HTTP 503', async () => {
    healthMock.mockResolvedValue(degradedResponse);

    render(<App />);

    expect(
      await screen.findByText('Servidor degradado', undefined, { timeout: 3000 }),
    ).toBeInTheDocument();
    expect(screen.getByText('Banco de dados indisponível')).toBeInTheDocument();
    expect(screen.getByText('Erro')).toBeInTheDocument();
    expect(screen.getByText(/banco de dados não está respondendo/i)).toBeInTheDocument();
  });

  it('exibe mensagem acionável para falha de rede', async () => {
    healthMock.mockRejectedValue(
      new ZunoApiError('network_error', 'Não foi possível conectar.', null),
    );

    render(<App />);

    expect(
      await screen.findByText('Não foi possível verificar o servidor', undefined, {
        timeout: 3000,
      }),
    ).toBeInTheDocument();
    expect(screen.getByText(/Inicie o servidor em http:\/\/127\.0\.0\.1:8080/)).toBeInTheDocument();
    expect(screen.getByText('network_error')).toBeInTheDocument();
  });

  it('refaz a chamada ao clicar em "Verificar novamente"', async () => {
    healthMock.mockResolvedValue(healthyResponse);

    render(<App />);

    expect(
      await screen.findByText('Servidor operacional', undefined, { timeout: 3000 }),
    ).toBeInTheDocument();
    expect(healthMock).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: 'Verificar novamente' }));

    await waitFor(() => {
      expect(healthMock).toHaveBeenCalledTimes(2);
    });

    expect(
      await screen.findByText('Servidor operacional', undefined, { timeout: 3000 }),
    ).toBeInTheDocument();
  });
});
