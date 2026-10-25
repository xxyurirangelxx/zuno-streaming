/**
 * Contratos compartilhados entre servidor, web e futuros clientes nativos.
 * Mantidos em sincronia manual com o contrato de saúde do M0.
 */

export type DatabaseHealthStatus = 'ok' | 'error';

export type ServiceHealthStatus = 'ok' | 'degraded';

export interface DatabaseHealth {
  status: DatabaseHealthStatus;
}

export interface HealthResponse {
  status: ServiceHealthStatus;
  service: string;
  version: string;
  uptime_seconds: number;
  database: DatabaseHealth;
}

export interface ApiErrorBody {
  code: string;
  message: string;
}
