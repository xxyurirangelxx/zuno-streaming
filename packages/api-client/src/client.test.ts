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

const user = { id: 'u1', username: 'ana', role: 'user' as const };

describe('createZunoClient autenticação', () => {
  it('envia cookies e Accept em me()', async () => {
    const fetchImpl = vi.fn<typeof fetch>().mockResolvedValue(jsonResponse(user));

    const client = createZunoClient({ fetchImpl });
    await expect(client.me()).resolves.toEqual(user);

    expect(fetchImpl).toHaveBeenCalledWith(
      '/api/v1/me',
      expect.objectContaining({
        method: 'GET',
        credentials: 'include',
        headers: expect.objectContaining({ Accept: 'application/json' }) as unknown,
      }),
    );
  });

  it('faz login com JSON e retorna a sessão', async () => {
    const fetchImpl = vi.fn<typeof fetch>().mockResolvedValue(jsonResponse({ user }));

    const client = createZunoClient({ fetchImpl });
    await expect(client.login({ username: 'ana', password: 'segredo123' })).resolves.toEqual({
      user,
    });

    expect(fetchImpl).toHaveBeenCalledWith(
      '/api/v1/auth/login',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ username: 'ana', password: 'segredo123' }),
        credentials: 'include',
        headers: expect.objectContaining({ 'Content-Type': 'application/json' }) as unknown,
      }),
    );
  });

  it('distingue 401 em ZunoApiError', async () => {
    const fetchImpl = vi
      .fn<typeof fetch>()
      .mockResolvedValue(jsonResponse({ code: 'unauthorized', message: 'Sessão inválida.' }, 401));

    const client = createZunoClient({ fetchImpl });

    await expect(client.me()).rejects.toMatchObject({
      name: 'ZunoApiError',
      code: 'unauthorized',
      status: 401,
    });
  });

  it('consulta o setup e reivindica o administrador', async () => {
    const fetchImpl = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(jsonResponse({ required: true }))
      .mockResolvedValueOnce(jsonResponse({ user: { ...user, role: 'admin' } }, 201));

    const client = createZunoClient({ fetchImpl });

    await expect(client.setupStatus()).resolves.toEqual({ required: true });
    await expect(
      client.setupClaim({ code: 'ABC12345', username: 'ana', password: 'segredo123' }),
    ).resolves.toMatchObject({ user: { role: 'admin' } });
  });

  it('resolve logout 204 sem corpo', async () => {
    const fetchImpl = vi.fn<typeof fetch>().mockResolvedValue(new Response(null, { status: 204 }));

    const client = createZunoClient({ fetchImpl });

    await expect(client.logout()).resolves.toBeUndefined();
    expect(fetchImpl).toHaveBeenCalledWith(
      '/api/v1/auth/logout',
      expect.objectContaining({ method: 'POST', credentials: 'include' }),
    );
  });
});

describe('createZunoClient biblioteca', () => {
  const track = {
    id: 't1',
    title: 'Faixa',
    duration_ms: 1000,
    track_no: 1,
    disc_no: 1,
    album: { id: 'a1', title: 'Álbum' },
    artists: [{ id: 'ar1', name: 'Artista' }],
    artwork_id: null,
    available: true,
  };

  it('pagina faixas com limit e cursor', async () => {
    const fetchImpl = vi
      .fn<typeof fetch>()
      .mockResolvedValue(jsonResponse({ items: [track], next_cursor: 'cursor-2' }));

    const client = createZunoClient({ fetchImpl });
    const page = await client.listTracks({ limit: 20, cursor: 'cursor-1' });

    expect(page.items).toHaveLength(1);
    expect(page.next_cursor).toBe('cursor-2');
    expect(fetchImpl).toHaveBeenCalledWith(
      '/api/v1/tracks?limit=20&cursor=cursor-1',
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('codifica o termo de busca', async () => {
    const fetchImpl = vi
      .fn<typeof fetch>()
      .mockResolvedValue(jsonResponse({ query: 'mpb', tracks: [], albums: [], artists: [] }));

    const client = createZunoClient({ fetchImpl });
    await client.search('mpb');

    expect(fetchImpl).toHaveBeenCalledWith(
      '/api/v1/search?q=mpb',
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('monta URLs de capa e stream na base configurada', () => {
    const client = createZunoClient({ baseUrl: 'http://127.0.0.1:8080/' });

    expect(client.artworkUrl('id 1', 'thumb')).toBe(
      'http://127.0.0.1:8080/api/v1/artworks/id%201?size=thumb',
    );
    expect(client.streamUrl('id 1')).toBe('http://127.0.0.1:8080/api/v1/tracks/id%201/stream');
  });
});
