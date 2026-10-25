import { describe, expect, it } from 'vitest';
import {
  describeHealthError,
  formatCheckedAt,
  formatDatabaseStatus,
  formatHealthStatus,
  formatUptime,
} from './format';

describe('formatUptime', () => {
  it('formata segundos, minutos, horas e dias', () => {
    expect(formatUptime(0)).toBe('0 s');
    expect(formatUptime(45)).toBe('45 s');
    expect(formatUptime(125)).toBe('2 min 5 s');
    expect(formatUptime(3725)).toBe('1 h 2 min 5 s');
    expect(formatUptime(90061)).toBe('1 d 1 h 1 min');
  });

  it('trata valores inválidos', () => {
    expect(formatUptime(-1)).toBe('—');
    expect(formatUptime(Number.NaN)).toBe('—');
  });
});

describe('formatHealthStatus / formatDatabaseStatus', () => {
  it('traduz os estados do contrato', () => {
    expect(formatHealthStatus('ok')).toBe('operacional');
    expect(formatHealthStatus('degraded')).toBe('degradado');
    expect(formatDatabaseStatus('ok')).toBe('Operacional');
    expect(formatDatabaseStatus('error')).toBe('Erro');
  });
});

describe('formatCheckedAt', () => {
  it('formata um horário local válido', () => {
    expect(formatCheckedAt(new Date('2026-10-25T15:04:05Z'))).toMatch(/^\d{2}:\d{2}:\d{2}$/);
  });
});

describe('describeHealthError', () => {
  it('fornece mensagens acionáveis por código', () => {
    expect(describeHealthError({ code: 'timeout', status: null })).toMatch(/tempo limite/i);
    expect(describeHealthError({ code: 'network_error', status: null })).toMatch(
      /127\.0\.0\.1:8080/,
    );
    expect(describeHealthError({ code: 'invalid_response', status: 200 })).toMatch(/formato/i);
    expect(describeHealthError({ code: 'http_error', status: 500 })).toMatch(/HTTP 500/);
    expect(describeHealthError({ code: 'unknown', status: null })).toMatch(/falha inesperada/i);
  });
});
