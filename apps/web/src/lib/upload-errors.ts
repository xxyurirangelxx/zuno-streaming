import { describeApiError } from './errors';

const OPUS_HINT =
  'A biblioteca aceita apenas Opus (.opus/.ogg). Converta o arquivo no cliente Windows/Tauri (M4) e envie novamente.';

/** Mensagem acionável para falhas de upload, sem expor caminhos do servidor. */
export function describeUploadError(error: { code: string; status: number | null }): string {
  if (error.status === 413 || error.code === 'too_large') {
    return 'Arquivo grande demais para o limite configurado no servidor. Reduza o arquivo ou aumente o limite do Zuno.';
  }

  if (
    error.status === 415 ||
    error.status === 422 ||
    error.code === 'unsupported_format' ||
    error.code === 'invalid_audio'
  ) {
    return OPUS_HINT;
  }

  if (error.status === 400 || error.code === 'invalid_filename') {
    return 'O nome do arquivo não é aceito pelo servidor. Renomeie o arquivo e tente novamente.';
  }

  if (error.status === 409 || error.code === 'duplicate') {
    return 'Este arquivo já existe na biblioteca. Nada foi sobrescrito.';
  }

  if (error.code === 'canceled') {
    return 'Envio cancelado.';
  }

  if (error.status === 401) {
    return 'Sua sessão expirou. Entre novamente para continuar o envio.';
  }

  return describeApiError(error);
}
