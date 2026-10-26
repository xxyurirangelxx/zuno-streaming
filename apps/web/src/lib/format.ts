import type {
  DatabaseHealthStatus,
  HistoryEventKind,
  ServiceHealthStatus,
} from '@zuno/shared-types';

const SECONDS_PER_MINUTE = 60;
const SECONDS_PER_HOUR = 3600;
const SECONDS_PER_DAY = 86400;

/**
 * Formata tempo de atividade em PT-BR, ex.: 125 -> "2 min 5 s".
 * Retorna "—" para valores inválidos ou negativos.
 */
export function formatUptime(totalSeconds: number): string {
  if (!Number.isFinite(totalSeconds) || totalSeconds < 0) {
    return '—';
  }

  const seconds = Math.floor(totalSeconds);
  const days = Math.floor(seconds / SECONDS_PER_DAY);
  const hours = Math.floor((seconds % SECONDS_PER_DAY) / SECONDS_PER_HOUR);
  const minutes = Math.floor((seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE);
  const remainingSeconds = seconds % SECONDS_PER_MINUTE;

  if (days > 0) {
    return `${days} d ${hours} h ${minutes} min`;
  }

  if (hours > 0) {
    return `${hours} h ${minutes} min ${remainingSeconds} s`;
  }

  if (minutes > 0) {
    return `${minutes} min ${remainingSeconds} s`;
  }

  return `${remainingSeconds} s`;
}

export function formatHealthStatus(status: ServiceHealthStatus): string {
  return status === 'ok' ? 'operacional' : 'degradado';
}

export function formatDatabaseStatus(status: DatabaseHealthStatus): string {
  return status === 'ok' ? 'Operacional' : 'Erro';
}

export function formatCheckedAt(date: Date): string {
  return new Intl.DateTimeFormat('pt-BR', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  }).format(date);
}

/** Texto acionável para falhas de conectividade, sem expor detalhes internos. */
export function describeHealthError(error: { code: string; status: number | null }): string {
  switch (error.code) {
    case 'timeout':
      return 'O servidor não respondeu dentro do tempo limite. Confirme se o Zuno está em execução e tente novamente.';
    case 'network_error':
      return 'Não foi possível conectar. Inicie o servidor em http://127.0.0.1:8080 e verifique se o proxy do Vite está ativo.';
    case 'invalid_response':
      return 'O servidor respondeu em um formato inesperado. Verifique se a versão do cliente corresponde à do servidor.';
    default:
      return typeof error.status === 'number'
        ? `O servidor respondeu com HTTP ${error.status}. Consulte os logs do servidor e tente novamente.`
        : 'Ocorreu uma falha inesperada ao consultar o servidor. Tente novamente.';
  }
}

/** Formata duração em milissegundos como `m:ss` ou `h:mm:ss`. */
export function formatDurationMs(durationMs: number | null): string {
  if (durationMs === null || !Number.isFinite(durationMs) || durationMs < 0) {
    return '--:--';
  }

  return formatClock(Math.floor(durationMs / 1000));
}

/** Formata segundos como `m:ss` ou `h:mm:ss`. */
export function formatClock(totalSeconds: number): string {
  if (!Number.isFinite(totalSeconds) || totalSeconds < 0) {
    return '0:00';
  }

  const seconds = Math.floor(totalSeconds);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remaining = seconds % 60;

  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, '0')}:${String(remaining).padStart(2, '0')}`;
  }

  return `${minutes}:${String(remaining).padStart(2, '0')}`;
}

/** Rótulo PT-BR do tipo de evento do histórico. */
export function formatHistoryEvent(kind: HistoryEventKind): string {
  return { play: 'Reprodução', skip: 'Pulo', complete: 'Concluída' }[kind];
}

/** Data e hora locais em PT-BR; retorna "—" para valores inválidos. */
export function formatDateTime(iso: string): string {
  const date = new Date(iso);

  if (Number.isNaN(date.getTime())) {
    return '—';
  }

  return new Intl.DateTimeFormat('pt-BR', {
    dateStyle: 'short',
    timeStyle: 'short',
  }).format(date);
}

/** Rótulo PT-BR do papel do usuário. */
export function formatRole(role: 'admin' | 'moderator' | 'user'): string {
  return { admin: 'Administrador', moderator: 'Moderador', user: 'Ouvinte' }[role];
}
