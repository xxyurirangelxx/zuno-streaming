## O que mudou

<!-- Resumo curto e objetivo. -->

## Milestone / issue

<!-- Ex.: M0 / ZUNO-002, closes #123 -->

## Testes executados

<!-- Cole a saída real dos comandos relevantes. -->

```text
$ cargo test --workspace
```

## Riscos

<!-- Regressões possíveis, migrações de dados, desempenho em Raspberry Pi 3, compatibilidade. -->

## Segurança / privacidade

- [ ] Bind padrão segue `127.0.0.1`; nenhuma exposição pública nova.
- [ ] Nenhum segredo, token, `.env` real ou caminho pessoal commitado.
- [ ] Nenhum arquivo de música, banco ou cache foi commitado.
- [ ] Logs não vazam dados sensíveis (senhas, tokens, caminhos privados).
- [ ] Permissões/autorização continuam mínimas e testadas.

## Checklist

- [ ] Testes automatizados adicionados/atualizados.
- [ ] `scripts/check.ps1` (ou `scripts/check.sh`) executado com sucesso.
- [ ] Documentação sincronizada com o comportamento.
