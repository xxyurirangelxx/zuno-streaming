import type { ApiErrorBody, HealthResponse } from '@zuno/shared-types';

const DEFAULT_TIMEOUT_MS = 5000;
const HEALTH_PATH = '/health';

export interface ZunoClientOptions {
  /** Base da API. Vazio (padrão) usa a mesma origem, via proxy do Vite. */
  baseUrl?: string;
  /** Injeção de `fetch` para testes. */
  fetchImpl?: typeof fetch;
  /** Timeout por requisição, em milissegundos. Padrão: 5000. */
  timeoutMs?: number;
}

export interface ZunoClient {
  health(signal?: AbortSignal): Promise<HealthResponse>;
}

export class ZunoApiError extends Error {
  readonly code: string;
  /** Status HTTP ou `null` para falha de rede/timeout. */
  readonly status: number | null;

  constructor(code: string, message: string, status: number | null, options?: ErrorOptions) {
    super(message, options);
    this.name = 'ZunoApiError';
    this.code = code;
    this.status = status;
  }
}

interface TimeoutSignal {
  signal: AbortSignal;
  cleanup: () => void;
  timedOut: () => boolean;
}

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

function timeoutError(): ZunoApiError {
  return new ZunoApiError(
    'timeout',
    'O servidor não respondeu dentro do tempo limite. Verifique a conexão e tente novamente.',
    null,
  );
}

function networkError(cause: unknown): ZunoApiError {
  return new ZunoApiError(
    'network_error',
    'Não foi possível conectar ao servidor. Verifique se ele está em execução.',
    null,
    { cause },
  );
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

  async function request<T>(
    path: string,
    parse: (body: unknown, response: Response) => T,
    signal?: AbortSignal,
  ): Promise<T> {
    const timeout = createTimeoutSignal(signal, timeoutMs);

    try {
      let response: Response;

      try {
        response = await fetchImpl(`${baseUrl}${path}`, {
          method: 'GET',
          headers: { Accept: 'application/json' },
          signal: timeout.signal,
        });
      } catch (error) {
        if (timeout.timedOut()) {
          throw timeoutError();
        }

        throw networkError(error);
      }

      let body: unknown;

      try {
        body = await response.json();
      } catch {
        throw new ZunoApiError(
          'invalid_response',
          'O servidor respondeu em um formato inesperado.',
          response.status,
        );
      }

      return parse(body, response);
    } finally {
      timeout.cleanup();
    }
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
            const apiError = parseApiErrorBody(body);

            throw new ZunoApiError(
              apiError?.code ?? 'http_error',
              apiError?.message ?? `O servidor respondeu com HTTP ${response.status}.`,
              response.status,
            );
          }

          throw new ZunoApiError(
            'invalid_response',
            'O servidor respondeu em um formato inesperado.',
            response.status,
          );
        },
        signal,
      );
    },
  };
}
