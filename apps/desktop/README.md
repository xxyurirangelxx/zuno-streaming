# Zuno Desktop (Tauri 2)

> **Status: scaffold M0 — não compilado nem validado.** Esta estrutura existe para o
> milestone M4 (MVP Windows). Ela não é construída no CI e não faz parte do workspace
> Rust da raiz.

## Estrutura

```text
apps/desktop/
├── README.md
└── src-tauri/
    ├── Cargo.toml
    ├── build.rs
    ├── tauri.conf.json
    ├── capabilities/default.json
    ├── icons/            # gerados por scripts/generate-icons.py
    └── src/
        ├── main.rs
        └── lib.rs
```

O app carrega o frontend de `apps/web` (Vite em `http://localhost:5173` em dev e
`../../web/dist` em build). O único comando exposto hoje é `app_version`.

## Pré-requisitos

- **Windows 10/11:** WebView2 Runtime (já presente na maioria dos sistemas) e
  Visual Studio Build Tools com "Desktop development with C++".
- **Linux:** `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`,
  `librsvg2-dev`, `patchelf`.
- Rust stable (ver `rust-toolchain.toml` na raiz) e Node.js 22+.
- Tauri CLI 2 (opcional, para rodar o app):
  `cargo install tauri-cli --version "^2"`.

## Ícones

Os ícones em `src-tauri/icons/` são gerados a partir de `logo.png` na raiz:

```bash
python scripts/generate-icons.py
```

O script usa Pillow (dependência opcional de desenvolvimento, nunca do build) e
não baixa nada da rede. Não use `tauri icon` neste momento: a CLI não está instalada
e o conjunto de arquivos já está versionado.

## Por que fora do workspace e do CI

- O crate depende de bibliotecas de sistema (WebView2, GTK/WebKitGTK) que não estão
  disponíveis em todos os runners/ambientes de desenvolvimento.
- Compilar um app desktop em todo `cargo test --workspace` tornaria o ciclo de
  desenvolvimento lento, sobretudo em máquinas modestas.
- A raiz declara `exclude = ["apps/desktop/src-tauri"]`. Quando o M4 começar, será
  avaliado adicionar um job de CI dedicado aos runners com as dependências corretas.

## O que falta no M4

- Integração real com a API do servidor (biblioteca, streaming, sessão).
- Player de áudio, cache/offline e atalhos de mídia.
- Empacotamento (MSI/NSIS), assinatura, atualização segura e testes em hardware real.
- Revisão do CSP e das capacidades mínimas por janela.
- Validação de acessibilidade e `reduced-motion`.

## Advertência de licença

O repositório ainda não possui uma licença de reutilização aprovada. Este crate é
marcado como `UNLICENSED`/`publish = false` até a decisão final.
