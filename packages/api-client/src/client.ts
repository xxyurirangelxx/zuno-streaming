import type {
  AddPlaylistItemRequest,
  AlbumListResponse,
  ApiErrorBody,
  ArtworkSize,
  ArtistListResponse,
  CreatedInvitation,
  CreateInvitationRequest,
  CreatePlaylistRequest,
  FavoriteKind,
  FavoritesResponse,
  HealthResponse,
  HistoryEventRequest,
  HistoryListParams,
  HistoryResponse,
  InvitationItem,
  ListPageParams,
  ListTracksParams,
  LoginRequest,
  MovePlaylistItemRequest,
  PlaylistDetail,
  PlaylistItemPosition,
  PlaylistSummary,
  RedeemInvitationRequest,
  SearchResponse,
  SessionResponse,
  SetupClaimRequest,
  SetupStatusResponse,
  TrackDetail,
  TrackListResponse,
  UpdatePlaylistRequest,
  UpdatePreferencesRequest,
  UpdatePrivacyRequest,
  UploadCompleteResponse,
  UploadCreated,
  UploadDetail,
  UploadProgress,
  User,
  UserPreferences,
  UserPrivacy,
} from '@zuno/shared-types';

const DEFAULT_TIMEOUT_MS = 5000;
const HEALTH_PATH = '/health';
const API_PREFIX = '/api/v1';

export interface ZunoClientOptions {
  /** Base da API. Vazio (padrão) usa a mesma origem, via proxy do Vite. */
  baseUrl?: string;
  /** Injeção de `fetch` para testes. */
  fetchImpl?: typeof fetch;
  /** Timeout por requisição, em milissegundos. Padrão: 5000. */
  timeoutMs?: number;
  /** Injeção do construtor de `XMLHttpRequest` para testes de upload. */
  xhrFactory?: () => XMLHttpRequest;
}

export interface UploadRequestOptions {
  /** Progresso real do envio do corpo binário. */
  onProgress?: (progress: UploadProgress) => void;
  signal?: AbortSignal;
}

export interface ZunoClient {
  health(signal?: AbortSignal): Promise<HealthResponse>;

  // Sessão e contas
  me(signal?: AbortSignal): Promise<User>;
  login(credentials: LoginRequest, signal?: AbortSignal): Promise<SessionResponse>;
  logout(signal?: AbortSignal): Promise<void>;
  refresh(signal?: AbortSignal): Promise<SessionResponse>;
  setupStatus(signal?: AbortSignal): Promise<SetupStatusResponse>;
  setupClaim(payload: SetupClaimRequest, signal?: AbortSignal): Promise<SessionResponse>;
  redeemInvitation(
    payload: RedeemInvitationRequest,
    signal?: AbortSignal,
  ): Promise<SessionResponse>;

  // Convites (admin)
  createInvitation(
    payload: CreateInvitationRequest,
    signal?: AbortSignal,
  ): Promise<CreatedInvitation>;
  listInvitations(signal?: AbortSignal): Promise<InvitationItem[]>;
  revokeInvitation(id: string, signal?: AbortSignal): Promise<void>;

  // Biblioteca
  listTracks(params?: ListTracksParams, signal?: AbortSignal): Promise<TrackListResponse>;
  listAlbums(params?: ListPageParams, signal?: AbortSignal): Promise<AlbumListResponse>;
  listArtists(params?: ListPageParams, signal?: AbortSignal): Promise<ArtistListResponse>;
  search(q: string, signal?: AbortSignal): Promise<SearchResponse>;
  getTrack(id: string, signal?: AbortSignal): Promise<TrackDetail>;

  // Playlists
  listPlaylists(signal?: AbortSignal): Promise<PlaylistSummary[]>;
  createPlaylist(payload: CreatePlaylistRequest, signal?: AbortSignal): Promise<PlaylistSummary>;
  getPlaylist(id: string, signal?: AbortSignal): Promise<PlaylistDetail>;
  updatePlaylist(
    id: string,
    payload: UpdatePlaylistRequest,
    signal?: AbortSignal,
  ): Promise<PlaylistSummary>;
  deletePlaylist(id: string, signal?: AbortSignal): Promise<void>;
  addPlaylistItem(
    id: string,
    payload: AddPlaylistItemRequest,
    signal?: AbortSignal,
  ): Promise<PlaylistItemPosition>;
  removePlaylistItem(id: string, trackId: string, signal?: AbortSignal): Promise<void>;
  movePlaylistItem(
    id: string,
    trackId: string,
    payload: MovePlaylistItemRequest,
    signal?: AbortSignal,
  ): Promise<void>;

  // Favoritos
  listFavorites(signal?: AbortSignal): Promise<FavoritesResponse>;
  setFavorite(
    kind: FavoriteKind,
    id: string,
    favorite: boolean,
    signal?: AbortSignal,
  ): Promise<void>;
  addFavorite(kind: FavoriteKind, id: string, signal?: AbortSignal): Promise<void>;
  removeFavorite(kind: FavoriteKind, id: string, signal?: AbortSignal): Promise<void>;

  // Preferências, privacidade e histórico
  getPreferences(signal?: AbortSignal): Promise<UserPreferences>;
  updatePreferences(
    payload: UpdatePreferencesRequest,
    signal?: AbortSignal,
  ): Promise<UserPreferences>;
  getPrivacy(signal?: AbortSignal): Promise<UserPrivacy>;
  updatePrivacy(payload: UpdatePrivacyRequest, signal?: AbortSignal): Promise<UserPrivacy>;
  recordHistoryEvent(payload: HistoryEventRequest, signal?: AbortSignal): Promise<void>;
  listHistory(params?: HistoryListParams, signal?: AbortSignal): Promise<HistoryResponse>;
  clearHistory(signal?: AbortSignal): Promise<void>;

  // Uploads
  createUpload(file: File, options?: UploadRequestOptions): Promise<UploadCreated>;
  getUpload(id: string, signal?: AbortSignal): Promise<UploadDetail>;
  completeUpload(id: string, signal?: AbortSignal): Promise<UploadCompleteResponse>;
  deleteUpload(id: string, signal?: AbortSignal): Promise<void>;

  // Mídia
  artworkUrl(id: string, size?: ArtworkSize): string;
  streamUrl(id: string): string;
}

export class ZunoApiError extends Error {
  readonly code: string;
  /** Status HTTP ou `null` para falha de rede/timeout. */
  readonly status: number | null;
  /** Corpo JSON já interpretado, quando houver (ex.: `track_id` em 409). */
  readonly body: unknown;

  constructor(
    code: string,
    message: string,
    status: number | null,
    options?: ErrorOptions & { body?: unknown },
  ) {
    super(message, options);
    this.name = 'ZunoApiError';
    this.code = code;
    this.status = status;
    this.body = options?.body ?? null;
  }
}

/** Sessão ausente/inválida: o app deve voltar ao login. */
export function isUnauthorizedError(error: unknown): boolean {
  return error instanceof ZunoApiError && error.status === 401;
}

interface TimeoutSignal {
  signal: AbortSignal;
  cleanup: () => void;
  timedOut: () => boolean;
}

interface RequestOptions {
  method?: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';
  body?: unknown;
  signal?: AbortSignal;
}

type Parser<T> = (body: unknown, response: Response) => T;

function createTimeoutSignal(external: AbortSignal | undefined, timeoutMs: number): TimeoutSignal {
  const controller = new AbortController();
  let didTimeout = false;

  const timer = setTimeout(() => {
    didTimeout = true;
    controller.abort(new DOMException('Tempo limite excedido', 'TimeoutError'));
  }, timeoutMs);

  const forwardAbort = () => {
    controller.abort(external?.reason);
  };

  if (external) {
    if (external.aborted) {
      forwardAbort();
    } else {
      external.addEventListener('abort', forwardAbort, { once: true });
    }
  }

  return {
    signal: controller.signal,
    cleanup: () => {
      clearTimeout(timer);
      external?.removeEventListener('abort', forwardAbort);
    },
    timedOut: () => didTimeout,
  };
}

function normalizeBaseUrl(baseUrl: string): string {
  return baseUrl.trim().replace(/\/+$/, '');
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isHealthResponse(value: unknown): value is HealthResponse {
  if (!isRecord(value) || !isRecord(value.database)) {
    return false;
  }

  return (
    (value.status === 'ok' || value.status === 'degraded') &&
    typeof value.service === 'string' &&
    typeof value.version === 'string' &&
    typeof value.uptime_seconds === 'number' &&
    (value.database.status === 'ok' || value.database.status === 'error')
  );
}

function parseApiErrorBody(value: unknown): ApiErrorBody | null {
  if (!isRecord(value) || typeof value.code !== 'string') {
    return null;
  }

  return {
    code: value.code,
    message: typeof value.message === 'string' ? value.message : value.code,
  };
}

function apiErrorFrom(body: unknown, status: number): ZunoApiError {
  const apiError = parseApiErrorBody(body);

  return new ZunoApiError(
    apiError?.code ?? 'http_error',
    apiError?.message ?? `O servidor respondeu com HTTP ${status}.`,
    status,
    { body },
  );
}

function invalidResponse(status: number): ZunoApiError {
  return new ZunoApiError(
    'invalid_response',
    'O servidor respondeu em um formato inesperado.',
    status,
  );
}

function timeoutError(): ZunoApiError {
  return new ZunoApiError(
    'timeout',
    'O servidor não respondeu dentro do tempo limite. Verifique a conexão e tente novamente.',
    null,
  );
}

function canceledError(): ZunoApiError {
  return new ZunoApiError('canceled', 'A operação foi cancelada.', null);
}

function networkError(cause: unknown): ZunoApiError {
  return new ZunoApiError(
    'network_error',
    'Não foi possível conectar ao servidor. Verifique se ele está em execução.',
    null,
    { cause },
  );
}

function requireOk(body: unknown, response: Response): void {
  if (!response.ok) {
    throw apiErrorFrom(body, response.status);
  }
}

function buildQuery(params: Record<string, string | number | undefined>): string {
  const search = new URLSearchParams();

  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== '') {
      search.set(key, String(value));
    }
  }

  const query = search.toString();
  return query ? `?${query}` : '';
}

function isThemePreference(value: unknown): value is UserPreferences['theme'] {
  return value === 'light' || value === 'dark' || value === 'system';
}

function parsePreferences(body: unknown, response: Response): UserPreferences {
  requireOk(body, response);

  if (!isRecord(body) || !isThemePreference(body.theme) || typeof body.language !== 'string') {
    throw invalidResponse(response.status);
  }

  return { theme: body.theme, language: body.language };
}

function parsePrivacy(body: unknown, response: Response): UserPrivacy {
  requireOk(body, response);

  if (
    !isRecord(body) ||
    typeof body.allow_cloud_history !== 'boolean' ||
    (body.history_retention_days !== null && typeof body.history_retention_days !== 'number')
  ) {
    throw invalidResponse(response.status);
  }

  return {
    allow_cloud_history: body.allow_cloud_history,
    history_retention_days: body.history_retention_days,
  };
}

function parseHistory(body: unknown, response: Response): HistoryResponse {
  requireOk(body, response);

  if (!isRecord(body) || !Array.isArray(body.items)) {
    throw invalidResponse(response.status);
  }

  return {
    items: body.items as HistoryResponse['items'],
    next_cursor: typeof body.next_cursor === 'string' ? body.next_cursor : null,
  };
}

export function createZunoClient(options: ZunoClientOptions = {}): ZunoClient {
  const baseUrl = normalizeBaseUrl(options.baseUrl ?? '');
  const fetchImpl =
    options.fetchImpl ??
    ((input: RequestInfo | URL, init?: RequestInit) => globalThis.fetch(input, init));
  const timeoutMs =
    options.timeoutMs !== undefined && Number.isFinite(options.timeoutMs) && options.timeoutMs > 0
      ? options.timeoutMs
      : DEFAULT_TIMEOUT_MS;
  const xhrFactory = options.xhrFactory ?? (() => new XMLHttpRequest());

  async function request<T>(
    path: string,
    parse: Parser<T>,
    requestOptions: RequestOptions = {},
  ): Promise<T> {
    const { method = 'GET', body, signal } = requestOptions;
    const timeout = createTimeoutSignal(signal, timeoutMs);
    const headers: Record<string, string> = { Accept: 'application/json' };

    if (body !== undefined) {
      headers['Content-Type'] = 'application/json';
    }

    let response: Response;

    try {
      try {
        response = await fetchImpl(`${baseUrl}${path}`, {
          method,
          headers,
          credentials: 'include',
          body: body === undefined ? undefined : JSON.stringify(body),
          signal: timeout.signal,
        });
      } catch (error) {
        if (timeout.timedOut()) {
          throw timeoutError();
        }

        if (signal?.aborted) {
          throw canceledError();
        }

        throw networkError(error);
      }

      if (response.status === 204) {
        return undefined as T;
      }

      const text = await response.text();
      let parsed: unknown = null;

      if (text.length > 0) {
        try {
          parsed = JSON.parse(text);
        } catch {
          throw invalidResponse(response.status);
        }
      }

      return parse(parsed, response);
    } finally {
      timeout.cleanup();
    }
  }

  function createUpload(
    file: File,
    uploadOptions: UploadRequestOptions = {},
  ): Promise<UploadCreated> {
    const { onProgress, signal } = uploadOptions;

    if (signal?.aborted) {
      return Promise.reject(canceledError());
    }

    return new Promise<UploadCreated>((resolve, reject) => {
      const xhr = xhrFactory();
      let settled = false;

      const cleanup = () => {
        signal?.removeEventListener('abort', onAbort);
      };

      const fail = (error: ZunoApiError) => {
        if (settled) {
          return;
        }

        settled = true;
        cleanup();
        reject(error);
      };

      const succeed = (value: UploadCreated) => {
        if (settled) {
          return;
        }

        settled = true;
        cleanup();
        resolve(value);
      };

      const onAbort = () => {
        xhr.abort();
        fail(canceledError());
      };

      xhr.open('POST', `${baseUrl}${API_PREFIX}/uploads`);
      xhr.withCredentials = true;
      xhr.setRequestHeader('Accept', 'application/json');
      xhr.setRequestHeader('X-Zuno-Filename', file.name);
      xhr.setRequestHeader('Content-Type', 'application/octet-stream');

      xhr.upload.onprogress = (event) => {
        if (!onProgress || event.lengthComputable === false) {
          return;
        }

        const percent = event.total > 0 ? Math.round((event.loaded / event.total) * 100) : 0;
        onProgress({ loaded: event.loaded, total: event.total, percent });
      };

      xhr.onload = () => {
        let parsed: unknown = null;

        if (xhr.responseText.length > 0) {
          try {
            parsed = JSON.parse(xhr.responseText);
          } catch {
            fail(invalidResponse(xhr.status));
            return;
          }
        }

        if (xhr.status < 200 || xhr.status >= 300) {
          fail(apiErrorFrom(parsed, xhr.status));
          return;
        }

        if (!isRecord(parsed) || typeof parsed.upload_id !== 'string') {
          fail(invalidResponse(xhr.status));
          return;
        }

        succeed(parsed as unknown as UploadCreated);
      };

      xhr.onerror = () => {
        fail(networkError(new Error('Falha de rede no upload.')));
      };

      xhr.ontimeout = () => {
        fail(timeoutError());
      };

      xhr.onabort = () => {
        fail(canceledError());
      };

      signal?.addEventListener('abort', onAbort, { once: true });

      try {
        xhr.send(file);
      } catch (error) {
        fail(networkError(error));
      }
    });
  }

  return {
    async health(signal?: AbortSignal): Promise<HealthResponse> {
      return request(
        HEALTH_PATH,
        (body, response) => {
          // O contrato de saúde usa 503 com o mesmo payload quando o banco falha.
          if (isHealthResponse(body)) {
            return body;
          }

          if (!response.ok) {
            throw apiErrorFrom(body, response.status);
          }

          throw invalidResponse(response.status);
        },
        { signal },
      );
    },

    async me(signal?: AbortSignal): Promise<User> {
      return request(
        `${API_PREFIX}/me`,
        (body, response) => {
          requireOk(body, response);
          if (!isRecord(body) || typeof body.id !== 'string') {
            throw invalidResponse(response.status);
          }
          return body as unknown as User;
        },
        { signal },
      );
    },

    async login(credentials: LoginRequest, signal?: AbortSignal): Promise<SessionResponse> {
      return request(
        `${API_PREFIX}/auth/login`,
        (body, response) => {
          requireOk(body, response);
          return body as SessionResponse;
        },
        { method: 'POST', body: credentials, signal },
      );
    },

    async logout(signal?: AbortSignal): Promise<void> {
      return request<void>(`${API_PREFIX}/auth/logout`, () => undefined, {
        method: 'POST',
        signal,
      });
    },

    async refresh(signal?: AbortSignal): Promise<SessionResponse> {
      return request(
        `${API_PREFIX}/auth/refresh`,
        (body, response) => {
          requireOk(body, response);
          return body as SessionResponse;
        },
        { method: 'POST', signal },
      );
    },

    async setupStatus(signal?: AbortSignal): Promise<SetupStatusResponse> {
      return request(
        `${API_PREFIX}/setup/status`,
        (body, response) => {
          requireOk(body, response);
          if (!isRecord(body) || typeof body.required !== 'boolean') {
            throw invalidResponse(response.status);
          }
          return { required: body.required };
        },
        { signal },
      );
    },

    async setupClaim(payload: SetupClaimRequest, signal?: AbortSignal): Promise<SessionResponse> {
      return request(
        `${API_PREFIX}/setup/claim`,
        (body, response) => {
          requireOk(body, response);
          return body as SessionResponse;
        },
        { method: 'POST', body: payload, signal },
      );
    },

    async redeemInvitation(
      payload: RedeemInvitationRequest,
      signal?: AbortSignal,
    ): Promise<SessionResponse> {
      return request(
        `${API_PREFIX}/invitations/redeem`,
        (body, response) => {
          requireOk(body, response);
          return body as SessionResponse;
        },
        { method: 'POST', body: payload, signal },
      );
    },

    async createInvitation(
      payload: CreateInvitationRequest,
      signal?: AbortSignal,
    ): Promise<CreatedInvitation> {
      return request(
        `${API_PREFIX}/invitations`,
        (body, response) => {
          requireOk(body, response);
          return body as CreatedInvitation;
        },
        { method: 'POST', body: payload, signal },
      );
    },

    async listInvitations(signal?: AbortSignal): Promise<InvitationItem[]> {
      return request(
        `${API_PREFIX}/invitations`,
        (body, response) => {
          requireOk(body, response);
          if (!Array.isArray(body)) {
            throw invalidResponse(response.status);
          }
          return body as InvitationItem[];
        },
        { signal },
      );
    },

    async revokeInvitation(id: string, signal?: AbortSignal): Promise<void> {
      return request<void>(`${API_PREFIX}/invitations/${encodeURIComponent(id)}`, () => undefined, {
        method: 'DELETE',
        signal,
      });
    },

    async listTracks(
      params: ListTracksParams = {},
      signal?: AbortSignal,
    ): Promise<TrackListResponse> {
      const query = buildQuery({
        limit: params.limit,
        cursor: params.cursor,
        album_id: params.album_id,
        artist_id: params.artist_id,
        q: params.q,
      });

      return request(
        `${API_PREFIX}/tracks${query}`,
        (body, response) => {
          requireOk(body, response);
          if (!isRecord(body) || !Array.isArray(body.items)) {
            throw invalidResponse(response.status);
          }
          return body as unknown as TrackListResponse;
        },
        { signal },
      );
    },

    async listAlbums(
      params: ListPageParams = {},
      signal?: AbortSignal,
    ): Promise<AlbumListResponse> {
      const query = buildQuery({ limit: params.limit, cursor: params.cursor });

      return request(
        `${API_PREFIX}/albums${query}`,
        (body, response) => {
          requireOk(body, response);
          if (!isRecord(body) || !Array.isArray(body.items)) {
            throw invalidResponse(response.status);
          }
          return body as unknown as AlbumListResponse;
        },
        { signal },
      );
    },

    async listArtists(
      params: ListPageParams = {},
      signal?: AbortSignal,
    ): Promise<ArtistListResponse> {
      const query = buildQuery({ limit: params.limit, cursor: params.cursor });

      return request(
        `${API_PREFIX}/artists${query}`,
        (body, response) => {
          requireOk(body, response);
          if (!isRecord(body) || !Array.isArray(body.items)) {
            throw invalidResponse(response.status);
          }
          return body as unknown as ArtistListResponse;
        },
        { signal },
      );
    },

    async search(q: string, signal?: AbortSignal): Promise<SearchResponse> {
      const query = buildQuery({ q });

      return request(
        `${API_PREFIX}/search${query}`,
        (body, response) => {
          requireOk(body, response);
          if (
            !isRecord(body) ||
            !Array.isArray(body.tracks) ||
            !Array.isArray(body.albums) ||
            !Array.isArray(body.artists)
          ) {
            throw invalidResponse(response.status);
          }
          return body as unknown as SearchResponse;
        },
        { signal },
      );
    },

    async getTrack(id: string, signal?: AbortSignal): Promise<TrackDetail> {
      return request(
        `${API_PREFIX}/tracks/${encodeURIComponent(id)}`,
        (body, response) => {
          requireOk(body, response);
          if (!isRecord(body) || typeof body.id !== 'string') {
            throw invalidResponse(response.status);
          }
          return body as unknown as TrackDetail;
        },
        { signal },
      );
    },

    async listPlaylists(signal?: AbortSignal): Promise<PlaylistSummary[]> {
      return request(
        `${API_PREFIX}/playlists`,
        (body, response) => {
          requireOk(body, response);
          if (!Array.isArray(body)) {
            throw invalidResponse(response.status);
          }
          return body as PlaylistSummary[];
        },
        { signal },
      );
    },

    async createPlaylist(
      payload: CreatePlaylistRequest,
      signal?: AbortSignal,
    ): Promise<PlaylistSummary> {
      return request(
        `${API_PREFIX}/playlists`,
        (body, response) => {
          requireOk(body, response);
          if (!isRecord(body) || typeof body.id !== 'string') {
            throw invalidResponse(response.status);
          }
          return body as unknown as PlaylistSummary;
        },
        { method: 'POST', body: payload, signal },
      );
    },

    async getPlaylist(id: string, signal?: AbortSignal): Promise<PlaylistDetail> {
      return request(
        `${API_PREFIX}/playlists/${encodeURIComponent(id)}`,
        (body, response) => {
          requireOk(body, response);
          if (!isRecord(body) || typeof body.id !== 'string' || !Array.isArray(body.items)) {
            throw invalidResponse(response.status);
          }
          return body as unknown as PlaylistDetail;
        },
        { signal },
      );
    },

    async updatePlaylist(
      id: string,
      payload: UpdatePlaylistRequest,
      signal?: AbortSignal,
    ): Promise<PlaylistSummary> {
      return request(
        `${API_PREFIX}/playlists/${encodeURIComponent(id)}`,
        (body, response) => {
          requireOk(body, response);
          if (!isRecord(body) || typeof body.id !== 'string') {
            throw invalidResponse(response.status);
          }
          return body as unknown as PlaylistSummary;
        },
        { method: 'PATCH', body: payload, signal },
      );
    },

    async deletePlaylist(id: string, signal?: AbortSignal): Promise<void> {
      return request<void>(`${API_PREFIX}/playlists/${encodeURIComponent(id)}`, () => undefined, {
        method: 'DELETE',
        signal,
      });
    },

    async addPlaylistItem(
      id: string,
      payload: AddPlaylistItemRequest,
      signal?: AbortSignal,
    ): Promise<PlaylistItemPosition> {
      return request(
        `${API_PREFIX}/playlists/${encodeURIComponent(id)}/items`,
        (body, response) => {
          requireOk(body, response);
          if (!isRecord(body) || typeof body.position !== 'number') {
            throw invalidResponse(response.status);
          }
          return { position: body.position };
        },
        { method: 'POST', body: payload, signal },
      );
    },

    async removePlaylistItem(id: string, trackId: string, signal?: AbortSignal): Promise<void> {
      return request<void>(
        `${API_PREFIX}/playlists/${encodeURIComponent(id)}/items/${encodeURIComponent(trackId)}`,
        () => undefined,
        { method: 'DELETE', signal },
      );
    },

    async movePlaylistItem(
      id: string,
      trackId: string,
      payload: MovePlaylistItemRequest,
      signal?: AbortSignal,
    ): Promise<void> {
      return request<void>(
        `${API_PREFIX}/playlists/${encodeURIComponent(id)}/items/${encodeURIComponent(trackId)}/position`,
        () => undefined,
        { method: 'PUT', body: payload, signal },
      );
    },

    async listFavorites(signal?: AbortSignal): Promise<FavoritesResponse> {
      return request(
        `${API_PREFIX}/favorites`,
        (body, response) => {
          requireOk(body, response);
          if (
            !isRecord(body) ||
            !Array.isArray(body.tracks) ||
            !Array.isArray(body.albums) ||
            !Array.isArray(body.artists)
          ) {
            throw invalidResponse(response.status);
          }
          return body as unknown as FavoritesResponse;
        },
        { signal },
      );
    },

    async setFavorite(
      kind: FavoriteKind,
      id: string,
      favorite: boolean,
      signal?: AbortSignal,
    ): Promise<void> {
      return request<void>(
        `${API_PREFIX}/favorites/${kind}/${encodeURIComponent(id)}`,
        () => undefined,
        { method: favorite ? 'PUT' : 'DELETE', signal },
      );
    },

    async addFavorite(kind: FavoriteKind, id: string, signal?: AbortSignal): Promise<void> {
      return request<void>(
        `${API_PREFIX}/favorites/${kind}/${encodeURIComponent(id)}`,
        () => undefined,
        { method: 'PUT', signal },
      );
    },

    async removeFavorite(kind: FavoriteKind, id: string, signal?: AbortSignal): Promise<void> {
      return request<void>(
        `${API_PREFIX}/favorites/${kind}/${encodeURIComponent(id)}`,
        () => undefined,
        { method: 'DELETE', signal },
      );
    },

    async getPreferences(signal?: AbortSignal): Promise<UserPreferences> {
      return request(`${API_PREFIX}/me/preferences`, parsePreferences, { signal });
    },

    async updatePreferences(
      payload: UpdatePreferencesRequest,
      signal?: AbortSignal,
    ): Promise<UserPreferences> {
      return request(`${API_PREFIX}/me/preferences`, parsePreferences, {
        method: 'PUT',
        body: payload,
        signal,
      });
    },

    async getPrivacy(signal?: AbortSignal): Promise<UserPrivacy> {
      return request(`${API_PREFIX}/me/privacy`, parsePrivacy, { signal });
    },

    async updatePrivacy(payload: UpdatePrivacyRequest, signal?: AbortSignal): Promise<UserPrivacy> {
      return request(`${API_PREFIX}/me/privacy`, parsePrivacy, {
        method: 'PUT',
        body: payload,
        signal,
      });
    },

    async recordHistoryEvent(payload: HistoryEventRequest, signal?: AbortSignal): Promise<void> {
      return request<void>(
        `${API_PREFIX}/me/history`,
        (body, response) => {
          requireOk(body, response);
          return undefined;
        },
        { method: 'POST', body: payload, signal },
      );
    },

    async listHistory(
      params: HistoryListParams = {},
      signal?: AbortSignal,
    ): Promise<HistoryResponse> {
      const query = buildQuery({ limit: params.limit, cursor: params.cursor });

      return request(`${API_PREFIX}/me/history${query}`, parseHistory, { signal });
    },

    async clearHistory(signal?: AbortSignal): Promise<void> {
      return request<void>(`${API_PREFIX}/me/history`, () => undefined, {
        method: 'DELETE',
        signal,
      });
    },

    createUpload,

    async getUpload(id: string, signal?: AbortSignal): Promise<UploadDetail> {
      return request(
        `${API_PREFIX}/uploads/${encodeURIComponent(id)}`,
        (body, response) => {
          requireOk(body, response);
          if (!isRecord(body) || typeof body.upload_id !== 'string') {
            throw invalidResponse(response.status);
          }
          return body as unknown as UploadDetail;
        },
        { signal },
      );
    },

    async completeUpload(id: string, signal?: AbortSignal): Promise<UploadCompleteResponse> {
      return request(
        `${API_PREFIX}/uploads/${encodeURIComponent(id)}/complete`,
        (body, response) => {
          requireOk(body, response);
          if (
            !isRecord(body) ||
            typeof body.upload_id !== 'string' ||
            typeof body.track_id !== 'string'
          ) {
            throw invalidResponse(response.status);
          }
          return body as unknown as UploadCompleteResponse;
        },
        { method: 'POST', signal },
      );
    },

    async deleteUpload(id: string, signal?: AbortSignal): Promise<void> {
      return request<void>(`${API_PREFIX}/uploads/${encodeURIComponent(id)}`, () => undefined, {
        method: 'DELETE',
        signal,
      });
    },

    artworkUrl(id: string, size: ArtworkSize = 'full'): string {
      return `${baseUrl}${API_PREFIX}/artworks/${encodeURIComponent(id)}?size=${encodeURIComponent(String(size))}`;
    },

    streamUrl(id: string): string {
      return `${baseUrl}${API_PREFIX}/tracks/${encodeURIComponent(id)}/stream`;
    },
  };
}
