/** Texto acionável para erros da API, sem expor detalhes internos. */
export function describeApiError(error: { code: string; status: number | null }): string {
  switch (error.code) {
    case 'timeout':
      return 'O servidor não respondeu dentro do tempo limite. Confirme se o Zuno está em execução e tente novamente.';
    case 'network_error':
      return 'Não foi possível conectar. Verifique se o servidor está em execução e se o proxy do Vite está ativo.';
    case 'invalid_response':
      return 'O servidor respondeu em um formato inesperado. Verifique a versão do servidor.';
    case 'unauthorized':
      return 'Sua sessão expirou. Entre novamente para continuar.';
    case 'forbidden':
      return 'Sua conta não tem permissão para esta ação.';
    case 'too_many_attempts':
    case 'rate_limited':
      return 'Muitas tentativas. Aguarde alguns minutos antes de tentar novamente.';
    case 'setup_not_required':
    case 'already_initialized':
      return 'Esta instância já foi configurada. Entre com sua conta.';
    default:
      break;
  }

  if (error.status === 401) {
    return 'Sua sessão expirou. Entre novamente para continuar.';
  }

  if (error.status === 403) {
    return 'Sua conta não tem permissão para esta ação.';
  }

  if (error.status === 429) {
    return 'Muitas tentativas. Aguarde alguns minutos antes de tentar novamente.';
  }

  return typeof error.status === 'number'
    ? `O servidor respondeu com HTTP ${error.status}. Tente novamente.`
    : 'Ocorreu uma falha inesperada. Tente novamente.';
}
