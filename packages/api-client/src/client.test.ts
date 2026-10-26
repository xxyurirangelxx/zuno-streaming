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

const playlist = {
  id: 'p1',
  name: 'Favoritas',
  visibility: 'private' as const,
  track_count: 2,
  updated_at: '2026-10-26T10:00:00Z',
};

const playlistTrack = {
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

describe('createZunoClient playlists', () => {
  it('lista, cria e abre playlists', async () => {
    const fetchImpl = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(jsonResponse([playlist]))
      .mockResolvedValueOnce(jsonResponse(playlist, 201))
      .mockResolvedValueOnce(jsonResponse({ ...playlist, items: [playlistTrack] }));

    const client = createZunoClient({ fetchImpl });

    await expect(client.listPlaylists()).resolves.toEqual([playlist]);
    await expect(client.createPlaylist({ name: 'Favoritas' })).resolves.toEqual(playlist);
    await expect(client.getPlaylist('p1')).resolves.toMatchObject({
      id: 'p1',
      items: [playlistTrack],
    });

    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      '/api/v1/playlists',
      expect.objectContaining({ method: 'GET', credentials: 'include' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      '/api/v1/playlists',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ name: 'Favoritas' }),
      }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      3,
      '/api/v1/playlists/p1',
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('propaga 409 de playlist duplicada como ZunoApiError', async () => {
    const fetchImpl = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(
        jsonResponse({ code: 'playlist_name_taken', message: 'Nome já usado.' }, 409),
      );

    const client = createZunoClient({ fetchImpl });

    await expect(client.createPlaylist({ name: 'Favoritas' })).rejects.toMatchObject({
      name: 'ZunoApiError',
      code: 'playlist_name_taken',
      status: 409,
    });
  });

  it('renomeia, exclui e manipula itens da playlist', async () => {
    const fetchImpl = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(jsonResponse({ ...playlist, name: 'Nova' }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }))
      .mockResolvedValueOnce(jsonResponse({ position: 3 }, 201))
      .mockResolvedValueOnce(new Response(null, { status: 204 }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));

    const client = createZunoClient({ fetchImpl });

    await expect(client.updatePlaylist('p1', { name: 'Nova' })).resolves.toMatchObject({
      name: 'Nova',
    });
    await expect(client.deletePlaylist('p1')).resolves.toBeUndefined();
    await expect(client.addPlaylistItem('p1', { track_id: 't1' })).resolves.toEqual({
      position: 3,
    });
    await expect(client.removePlaylistItem('p1', 't1')).resolves.toBeUndefined();
    await expect(client.movePlaylistItem('p1', 't1', { position: 1 })).resolves.toBeUndefined();

    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      '/api/v1/playlists/p1',
      expect.objectContaining({
        method: 'PATCH',
        body: JSON.stringify({ name: 'Nova' }),
      }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      '/api/v1/playlists/p1',
      expect.objectContaining({ method: 'DELETE' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      3,
      '/api/v1/playlists/p1/items',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ track_id: 't1' }),
      }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      4,
      '/api/v1/playlists/p1/items/t1',
      expect.objectContaining({ method: 'DELETE' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      5,
      '/api/v1/playlists/p1/items/t1/position',
      expect.objectContaining({
        method: 'PUT',
        body: JSON.stringify({ position: 1 }),
      }),
    );
  });

  it('propaga 409 de faixa duplicada na playlist', async () => {
    const fetchImpl = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(
        jsonResponse({ code: 'playlist_item_duplicate', message: 'Faixa já presente.' }, 409),
      );

    const client = createZunoClient({ fetchImpl });

    await expect(client.addPlaylistItem('p1', { track_id: 't1' })).rejects.toMatchObject({
      code: 'playlist_item_duplicate',
      status: 409,
    });
  });
});

describe('createZunoClient favoritos', () => {
  it('lista favoritos e alterna PUT/DELETE idempotentes', async () => {
    const fetchImpl = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(jsonResponse({ tracks: [playlistTrack], albums: [], artists: [] }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));

    const client = createZunoClient({ fetchImpl });

    await expect(client.listFavorites()).resolves.toMatchObject({
      tracks: [playlistTrack],
    });
    await expect(client.setFavorite('track', 't1', true)).resolves.toBeUndefined();
    await expect(client.setFavorite('track', 't1', false)).resolves.toBeUndefined();

    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      '/api/v1/favorites',
      expect.objectContaining({ method: 'GET' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      '/api/v1/favorites/track/t1',
      expect.objectContaining({ method: 'PUT' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      3,
      '/api/v1/favorites/track/t1',
      expect.objectContaining({ method: 'DELETE' }),
    );
  });

  it('expõe addFavorite e removeFavorite', async () => {
    const fetchImpl = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(new Response(null, { status: 204 }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));

    const client = createZunoClient({ fetchImpl });

    await expect(client.addFavorite('album', 'a1')).resolves.toBeUndefined();
    await expect(client.removeFavorite('artist', 'ar1')).resolves.toBeUndefined();

    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      '/api/v1/favorites/album/a1',
      expect.objectContaining({ method: 'PUT' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      '/api/v1/favorites/artist/ar1',
      expect.objectContaining({ method: 'DELETE' }),
    );
  });
});

class FakeXhr {
  static instances: FakeXhr[] = [];

  method = '';
  url = '';
  withCredentials = false;
  sentBody: unknown = null;
  aborted = false;
  status = 0;
  responseText = '';
  headers: Record<string, string> = {};
  upload = {
    onprogress: null as ((event: ProgressEvent) => void) | null,
  };
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  ontimeout: (() => void) | null = null;
  onabort: (() => void) | null = null;

  constructor() {
    FakeXhr.instances.push(this);
  }

  open(method: string, url: string): void {
    this.method = method;
    this.url = url;
  }

  setRequestHeader(name: string, value: string): void {
    this.headers[name] = value;
  }

  send(body: unknown): void {
    this.sentBody = body;
  }

  abort(): void {
    this.aborted = true;
    this.onabort?.();
  }

  progress(loaded: number, total: number): void {
    this.upload.onprogress?.({
      lengthComputable: true,
      loaded,
      total,
    } as ProgressEvent);
  }

  respond(status: number, body: unknown): void {
    this.status = status;
    this.responseText = body === null ? '' : JSON.stringify(body);
    this.onload?.();
  }
}

function createXhrClient() {
  const xhrs: FakeXhr[] = [];
  const client = createZunoClient({
    xhrFactory: () => {
      const xhr = new FakeXhr();
      xhrs.push(xhr);
      return xhr as unknown as XMLHttpRequest;
    },
  });

  return { client, xhrs };
}

const createdUpload = {
  upload_id: 'up1',
  original_filename: 'faixa.opus',
  size_bytes: 10,
  sha256: 'abc',
  status: 'staged' as const,
};

describe('createZunoClient uploads', () => {
  it('envia o arquivo binário com nome, credenciais e progresso real', async () => {
    const { client, xhrs } = createXhrClient();
    const file = new File(['conteudo'], 'faixa.opus', { type: 'audio/ogg' });
    const onProgress = vi.fn();

    const promise = client.createUpload(file, { onProgress });
    const xhr = xhrs[0];

    expect(xhr).toBeDefined();
    expect(xhr?.method).toBe('POST');
    expect(xhr?.url).toBe('/api/v1/uploads');
    expect(xhr?.withCredentials).toBe(true);
    expect(xhr?.headers['X-Zuno-Filename']).toBe('faixa.opus');
    expect(xhr?.headers['Content-Type']).toBe('application/octet-stream');
    expect(xhr?.sentBody).toBe(file);

    xhr?.progress(50, 100);
    expect(onProgress).toHaveBeenCalledWith({ loaded: 50, total: 100, percent: 50 });

    xhr?.respond(201, createdUpload);
    await expect(promise).resolves.toEqual(createdUpload);
  });

  it('mapeia 413 com corpo JSON para ZunoApiError', async () => {
    const { client, xhrs } = createXhrClient();
    const promise = client.createUpload(new File(['x'], 'grande.opus'));

    xhrs[0]?.respond(413, { code: 'too_large', message: 'Arquivo grande demais.' });

    await expect(promise).rejects.toMatchObject({
      name: 'ZunoApiError',
      code: 'too_large',
      status: 413,
      body: { code: 'too_large', message: 'Arquivo grande demais.' },
    });
  });

  it('aborta o XHR e resolve como canceled ao cancelar', async () => {
    const { client, xhrs } = createXhrClient();
    const controller = new AbortController();
    const promise = client.createUpload(new File(['x'], 'a.opus'), {
      signal: controller.signal,
    });

    controller.abort();

    expect(xhrs[0]?.aborted).toBe(true);
    await expect(promise).rejects.toMatchObject({ code: 'canceled', status: null });
  });

  it('consulta, completa e remove uploads de staging', async () => {
    const fetchImpl = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(
        jsonResponse({
          ...createdUpload,
          status: 'staged',
          track_id: null,
          error: null,
        }),
      )
      .mockResolvedValueOnce(jsonResponse({ upload_id: 'up1', status: 'indexed', track_id: 't1' }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));

    const client = createZunoClient({ fetchImpl });

    await expect(client.getUpload('up1')).resolves.toMatchObject({ status: 'staged' });
    await expect(client.completeUpload('up1')).resolves.toEqual({
      upload_id: 'up1',
      status: 'indexed',
      track_id: 't1',
    });
    await expect(client.deleteUpload('up1')).resolves.toBeUndefined();

    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      '/api/v1/uploads/up1',
      expect.objectContaining({ method: 'GET' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      '/api/v1/uploads/up1/complete',
      expect.objectContaining({ method: 'POST' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      3,
      '/api/v1/uploads/up1',
      expect.objectContaining({ method: 'DELETE' }),
    );
  });

  it('preserva o corpo do 409 de duplicata', async () => {
    const fetchImpl = vi.fn<typeof fetch>().mockResolvedValue(
      jsonResponse(
        {
          upload_id: 'up1',
          status: 'duplicate',
          track_id: 't1',
        },
        409,
      ),
    );

    const client = createZunoClient({ fetchImpl });

    await expect(client.completeUpload('up1')).rejects.toMatchObject({
      status: 409,
      body: { upload_id: 'up1', status: 'duplicate', track_id: 't1' },
    });
  });
});
