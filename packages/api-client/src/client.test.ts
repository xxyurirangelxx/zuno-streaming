import type { HealthResponse } from '@zuno/shared-types';
import { describe, expect, it, vi } from 'vitest';
import { createZunoClient, type ZunoApiError } from './client';

const healthyResponse: HealthResponse = {
  status: 'ok',
  service: 'zuno-server',
  version: '0.1.0',
  uptime_seconds: 12,
  database: { status: 'ok' },
};

const degradedResponse: HealthResponse = {
  ...healthyResponse,
  status: 'degraded',
  database: { status: 'error' },
};

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

describe('createZunoClient health', () => {
  it('retorna o payload em HTTP 200', async () => {
    const fetchImpl = vi.fn<typeof fetch>().mockResolvedValue(jsonResponse(healthyResponse));

    const client = createZunoClient({ fetchImpl });
    await expect(client.health()).resolves.toEqual(healthyResponse);
    expect(fetchImpl).toHaveBeenCalledWith('/health', expect.objectContaining({ method: 'GET' }));
  });

  it('normaliza a barra final da base URL', async () => {
    const fetchImpl = vi.fn<typeof fetch>().mockResolvedValue(jsonResponse(healthyResponse));

    const client = createZunoClient({ baseUrl: 'http://127.0.0.1:8080/', fetchImpl });
    await client.health();

    expect(fetchImpl).toHaveBeenCalledWith(
      'http://127.0.0.1:8080/health',
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('retorna estado degradado em HTTP 503 com payload de saúde', async () => {
    const fetchImpl = vi.fn<typeof fetch>().mockResolvedValue(jsonResponse(degradedResponse, 503));

    const client = createZunoClient({ fetchImpl });
    await expect(client.health()).resolves.toEqual(degradedResponse);
  });

  it('lança ZunoApiError com o corpo de erro em HTTP 500', async () => {
    const fetchImpl = vi
      .fn<typeof fetch>()
      .mockResolvedValue(
        jsonResponse({ code: 'database_unavailable', message: 'Banco indisponível.' }, 500),
      );

    const client = createZunoClient({ fetchImpl });

    await expect(client.health()).rejects.toMatchObject({
      name: 'ZunoApiError',
      code: 'database_unavailable',
      message: 'Banco indisponível.',
      status: 500,
    } satisfies Partial<ZunoApiError>);
  });

  it('usa http_error quando o corpo de erro não tem o contrato esperado', async () => {
    const fetchImpl = vi
      .fn<typeof fetch>()
      .mockResolvedValue(jsonResponse({ detail: 'falhou' }, 500));

    const client = createZunoClient({ fetchImpl });

    await expect(client.health()).rejects.toMatchObject({ code: 'http_error', status: 500 });
  });

  it('rejeita corpo não-JSON como invalid_response', async () => {
    const fetchImpl = vi
      .fn<typeof fetch>()
      .mockResolvedValue(new Response('não é json', { status: 200 }));

    const client = createZunoClient({ fetchImpl });

    await expect(client.health()).rejects.toMatchObject({
      code: 'invalid_response',
      status: 200,
    });
  });

  it('rejeita payload de saúde incompleto como invalid_response', async () => {
    const fetchImpl = vi
      .fn<typeof fetch>()
      .mockResolvedValue(jsonResponse({ status: 'ok', service: 'zuno-server' }));

    const client = createZunoClient({ fetchImpl });

    await expect(client.health()).rejects.toMatchObject({ code: 'invalid_response' });
  });

  it('converte falha de rede em network_error', async () => {
    const fetchImpl = vi.fn<typeof fetch>().mockRejectedValue(new TypeError('fetch failed'));

    const client = createZunoClient({ fetchImpl });

    await expect(client.health()).rejects.toMatchObject({
      code: 'network_error',
      status: null,
    });
  });

  it('converte estouro de tempo em timeout', async () => {
    const fetchImpl = vi.fn<typeof fetch>(
      (_input, init) =>
        new Promise<Response>((_resolve, reject) => {
          init?.signal?.addEventListener(
            'abort',
            () => reject(new DOMException('Aborted', 'AbortError')),
            { once: true },
          );
        }),
    );

    const client = createZunoClient({ fetchImpl, timeoutMs: 20 });

    await expect(client.health()).rejects.toMatchObject({ code: 'timeout', status: null });
  });
});
