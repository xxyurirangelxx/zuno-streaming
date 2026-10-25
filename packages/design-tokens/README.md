# @zuno/design-tokens

Tokens CSS iniciais do Zuno, derivados das cores da logo (ciano, azul, índigo,
violeta, magenta e navy).

> **Provisório:** os valores foram amostrados da logo e ajustados para contraste.
> A validação visual e a definição final dos tokens acontecem no M2.

## Uso

```css
@import '@zuno/design-tokens/tokens.css';
```

O pacote expõe:

- `@zuno/design-tokens/tokens.css` — variáveis `--zuno-*` para tema claro,
  escuro, `prefers-reduced-motion`, `prefers-reduced-transparency` e
  `prefers-contrast`.
- `@zuno/design-tokens/tokens.json` — os mesmos valores em JSON para tooling
  futuro (build, documentação e testes de contraste).

## Temas e acessibilidade

- Tema escuro explícito: `data-theme="dark"`; claro explícito:
  `data-theme="light"`; sem atributo, segue `prefers-color-scheme`.
- Movimento: `data-reduced-motion="true"` zera `--zuno-motion-duration`.
- Transparência: `data-reduced-transparency="true"` troca o vidro por uma
  superfície opaca.
- Contraste: `data-contrast="more"` reforça bordas e texto secundário.
