# Ambiente de desenvolvimento

Guia para subir o Zuno localmente no marco **M0** (backend + página de conectividade web). Não cobre instalação em produção, Raspberry Pi, Docker, Tauri ou deploy público — tudo isso é planejado para milestones posteriores.

Tempo esperado: alguns minutos, dependendo do download da toolchain Rust e das dependências npm.

## 1. Pré-requisitos

| Ferramenta | Versão mínima | Observação |
|---|---|---|
| Rust (stable, MSVC no Windows) | 1.88 | `rust-toolchain.toml` fixa o canal `stable` com `rustfmt` e `clippy` |
| Node.js | 22.12 | `package.json` declara `engines.node >= 22.12.0` |
| npm | 10 | Vem com o Node; usado como gerenciador de workspaces |
| Git | qualquer recente | Para clonar o repositório |
| Python + Pillow | opcional | Apenas para regenerar ícones do Tauri (`scripts/generate-icons.py`) |

Não é necessário instalar o servidor SQLite: o driver compila o SQLite embutido.

### 1.1 Windows 10/11 (ambiente primário)

1. **Visual Studio Build Tools** com a carga de trabalho **"Desenvolvimento para desktop com C++"** (inclui MSVC e o linker `link.exe`) e o Windows SDK. Sem isso o Rust falha no link com `link.exe not found`.
2. **Rust via rustup**: instale de <https://rustup.rs> e confirme que a toolchain padrão é `stable-x86_64-pc-windows-msvc`:

   ```powershell
   rustup default stable-msvc
   rustc --version
   cargo --version
   ```

3. **Node.js 22.12+** (instalador oficial ou `winget install OpenJS.NodeJS.LTS`):

   ```powershell
   node --version
   npm --version
   ```

4. **WebView2 Runtime** só é necessário para o app Tauri (M4). O Windows 10/11 atualizado normalmente já o possui; nada a fazer no M0.
5. Use **PowerShell** na raiz do repositório. Se `cargo` não for reconhecido após instalar, feche e reabra o terminal.

### 1.2 Linux (x86-64 ou ARM)

```bash
# Debian/Ubuntu — compilador C é necessário para o SQLite embutido
sudo apt update
sudo apt install -y build-essential pkg-config curl git

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustc --version && cargo --version

# Node.js 22.12+ (exemplo com nvm; use o gerenciador de sua preferência)
nvm install 22 && nvm use 22
node --version && npm --version
```

Em distribuições baseadas em RPM, o equivalente é `gcc`, `gcc-c++`, `make` e `pkg-config`. Bibliotecas gráficas (`libwebkit2gtk`, `libgtk-3-dev`) só serão necessárias para o Tauri (M4) — veja `apps/desktop/README.md`.

## 2. Obter o código e instalar dependências

```bash
git clone <URL-do-repositorio> zuno-streaming
cd zuno-streaming
```

Backend Rust (compila na primeira execução; não instala nada globalmente):

```bash
cargo fetch
```

Frontend (instala os workspaces `apps/web` e `packages/*`):

```bash
npm install
```

> **Lockfile.** O `package-lock.json` está versionado no repositório; use `npm ci` para instalar exatamente as versões travadas (é o comando usado no CI). Use `npm install` apenas quando estiver mudando dependências de propósito.

## 3. Variáveis de ambiente

O servidor lê variáveis de ambiente do **processo**. Não há carregamento automático de `.env` no M0: `.env.example` serve como referência/documentação, não como arquivo lido pela aplicação. Exporte as variáveis no terminal (ou use o mecanismo do seu shell) antes de rodar `cargo run`.

| Variável | Padrão | Efeito |
|---|---|---|
| `ZUNO_BIND_ADDR` | `127.0.0.1:8080` | Endereço `IP:PORTA` de escuta. Valor inválido aborta a inicialização |
| `ZUNO_DATABASE_URL` | `sqlite://data/zuno.db` | Banco SQLite; arquivo e diretório são criados se não existirem. Aceita `sqlite::memory:` em testes |
| `ZUNO_MUSIC_DIR` | `./data/music` | Biblioteca **somente Opus**. O scan indexa sem mover/apagar; precisa existir |
| `ZUNO_ARTWORK_DIR` | `./data/artwork` | Cache de capas extraídas (criado automaticamente) |
| `ZUNO_MAX_AUDIO_FILE_BYTES` | `524288000` | Limite por arquivo no scan (1 KiB–10 GiB) |
| `ZUNO_SCAN_ON_STARTUP` | `false` | Varre a biblioteca na inicialização do servidor |
| `ZUNO_DB_MAX_CONNECTIONS` | `4` | Conexões máximas no pool; aceita 1–32. Mantenha baixo em hardware modesto |
| `RUST_LOG` | `info,zuno_server=debug` | Filtro de logs do `tracing` (`trace`, `debug`, `info`, `warn`, `error`) |
| `VITE_ZUNO_API_BASE_URL` | vazio | Base da API usada pelo frontend. Vazio = mesma origem, via proxy do Vite |
| `ZUNO_DEV_API_TARGET` | `http://127.0.0.1:8080` | Destino do proxy `/health` no servidor de desenvolvimento do Vite. Útil quando a porta 8080 já está ocupada por outro software |

Exemplos:

```powershell
$env:RUST_LOG = "debug"
$env:ZUNO_BIND_ADDR = "127.0.0.1:8080"
cargo run -p zuno-server
```

```bash
export RUST_LOG="debug"
export ZUNO_BIND_ADDR="127.0.0.1:8080"
cargo run -p zuno-server
```

O padrão de bind é **loopback**. Se você alterar `ZUNO_BIND_ADDR` para um IP de rede, a instância passa a ser acessível por outros dispositivos — não existe autenticação nem TLS no M0. Não exponha publicamente.

## 4. Rodar backend e frontend (dois terminais)

**Terminal 1 — backend:**

```bash
cargo run -p zuno-server
```

Saída esperada (resumida):

```text
INFO zuno_server: migrations do SQLite aplicadas applied=1
INFO zuno_server: servidor Zuno escutando address=127.0.0.1:8080 version=0.1.0
```

**Terminal 2 — frontend:**

```bash
npm run dev
```

Acesse <http://127.0.0.1:5173>. O Vite faz proxy de `/health` para `http://127.0.0.1:8080`; a página mostra o estado do servidor, a versão, o uptime e o estado do banco, com botão de nova consulta.

### 4.1 Varrer a biblioteca

Coloque arquivos **Opus** em `ZUNO_MUSIC_DIR` (outros formatos são ignorados e reportados, nunca convertidos) e rode:

```bash
cargo run -p zuno-server -- scan
```

O comando imprime um relatório JSON com `files_seen`, `files_indexed`, `files_unchanged`, `files_skipped`,
`files_failed`, `files_missing` e a lista de `issues` (`unsupported_format`, `corrupt_file`, `too_large`,
`symlink_skipped`, `outside_root`, `duplicate_exact`, `read_error`). O scan é idempotente: rodar de novo não
duplica faixas e não relê arquivos inalterados. Arquivo removido vira `available = 0`; nada é apagado do banco.

### 4.2 Verificar a API diretamente

```bash
curl http://127.0.0.1:8080/health
```

```powershell
Invoke-RestMethod http://127.0.0.1:8080/health | ConvertTo-Json
```

Resposta esperada:

```json
{
  "status": "ok",
  "service": "zuno-server",
  "version": "0.1.0",
  "uptime_seconds": 12,
  "database": { "status": "ok" }
}
```

Se o banco não responder, o status HTTP é **503** com `"status": "degraded"` e `"database": { "status": "error" }`. Rota inexistente devolve `404` com `{ "code": "not_found", "message": "..." }`.

## 5. Portas

| Porta | Serviço | Configuração |
|---|---|---|
| `8080` | API do Zuno (`zuno-server`) | `ZUNO_BIND_ADDR` |
| `5173` | Vite dev server (`npm run dev`) | fixa em `apps/web/vite.config.ts` (`strictPort: true`) |

O proxy de `/health` aponta para `http://127.0.0.1:8080` por padrão. Se a porta 8080 estiver ocupada (no Windows, alguns aplicativos de terceiros a utilizam), rode o backend em outra porta e aponte o proxy:

```powershell
$env:ZUNO_BIND_ADDR = "127.0.0.1:18081"
$env:ZUNO_DEV_API_TARGET = "http://127.0.0.1:18081"
```

```bash
export ZUNO_BIND_ADDR="127.0.0.1:18081"
export ZUNO_DEV_API_TARGET="http://127.0.0.1:18081"
```

Alternativamente, defina `VITE_ZUNO_API_BASE_URL` para apontar direto à API; nesse caso o navegador passa a fazer requisição entre origens e o servidor não habilita CORS no M0 (portanto o proxy continua sendo o caminho recomendado em desenvolvimento).

## 6. Comandos úteis

| Objetivo | Comando |
|---|---|
| Subir o backend | `cargo run -p zuno-server` |
| Testes Rust | `cargo test --workspace` |
| Formatação Rust | `cargo fmt --all` / verificar: `cargo fmt --all --check` |
| Lint Rust | `cargo clippy --workspace --all-targets -- -D warnings` |
| Instalar deps do frontend | `npm install` (ou `npm ci` com lockfile) |
| Dev web | `npm run dev` |
| Testes web | `npm test` (watch: `npm run test:watch` — disponível nos workspaces) |
| Typecheck / lint / build | `npm run typecheck`, `npm run lint`, `npm run build` |
| Pipeline completa | `scripts/check.sh` (bash) ou `scripts/check.ps1` (PowerShell) |

## 7. Reset seguro do banco de desenvolvimento

O banco de desenvolvimento é **descartável**: contém apenas `_sqlx_migrations` e a tabela `zuno_meta` no M0. As migrations são reaplicadas na próxima inicialização.

**Nunca apague arquivos de música.** O diretório padrão da biblioteca é `./data/music`, que fica **dentro** de `data/` — por isso nunca remova `data/` inteiro. Apague apenas os arquivos do banco.

1. Pare o servidor (`Ctrl+C`) e confirme que não há outra instância rodando. Não copie nem apague o banco com o servidor em execução.
2. Remova apenas os arquivos do banco:

   ```powershell
   # PowerShell, a partir da raiz do repositório
   Remove-Item -Force data\zuno.db, data\zuno.db-wal, data\zuno.db-shm -ErrorAction SilentlyContinue
   ```

   ```bash
   # bash
   rm -f data/zuno.db data/zuno.db-wal data/zuno.db-shm
   ```

3. Suba o servidor novamente: o arquivo é recriado e a migration `0001_bootstrap` é aplicada.

Se você usa um caminho customizado em `ZUNO_DATABASE_URL`, apague o arquivo correspondente (e os sufixos `-wal`/`-shm`) em vez de `data/zuno.db`. Para preservar dados, faça backup do arquivo antes; não há API de backup no M0.

## 8. Troubleshooting

### Porta 8080 ocupada

Sintoma: `não foi possível escutar em 127.0.0.1:8080` / `Address already in use`.

> No Windows 10/11 é comum outro software ocupar a 8080 (por exemplo, componentes auxiliares de aplicativos de jogos). Nesse caso, rode o backend em `127.0.0.1:18081` e exporte `ZUNO_DEV_API_TARGET=http://127.0.0.1:18081` antes de `npm run dev`.

```powershell
# Windows: descobrir o processo
Get-NetTCPConnection -LocalPort 8080 -State Listen | Select-Object OwningProcess
Get-Process -Id <PID>
```

```bash
# Linux
ss -ltnp | grep ':8080'
# ou
lsof -i :8080
```

Encerre o processo dono da porta ou use outra porta (`ZUNO_BIND_ADDR=127.0.0.1:8090`), lembrando de ajustar o proxy do Vite.

### Porta 5173 ocupada

Sintoma: o Vite encerra por causa de `strictPort: true`. Encerre o processo anterior (outro `npm run dev`) — o servidor não escolhe outra porta automaticamente.

### `database is locked` / erros de escrita no SQLite

- Provavelmente há **duas instâncias** do servidor usando o mesmo arquivo. Encerre uma.
- O pool usa `busy_timeout` de 5 s; operações longas (backup manual, cópia do arquivo, antivírus) podem exceder. Pare o servidor antes de copiar/mover o banco.
- Não edite o banco com ferramentas externas enquanto o servidor roda.

### `/health` retorna 503

- O `SELECT 1` no pool falhou: verifique caminho e permissões de `ZUNO_DATABASE_URL`, espaço em disco e se o arquivo/diretório existe.
- Veja o log do servidor (`RUST_LOG=debug`).

### Frontend não conecta (`network_error` / status 500 no proxy)

- O backend não está rodando ou está em outra porta.
- Confirme com `curl http://127.0.0.1:8080/health`.
- Se você definiu `VITE_ZUNO_API_BASE_URL` com outra origem, o navegador faz requisição cross-origin e o M0 não habilita CORS; deixe a variável vazia para usar o proxy.

### `link.exe not found` (Windows)

Instale o **Visual Studio Build Tools** com "Desenvolvimento para desktop com C++" e o Windows SDK; rode `rustup default stable-msvc`. Se o erro persistir, abra o terminal "x64 Native Tools Command Prompt for VS" e tente de novo.

### `node: bad option` / engine incompatível

Node abaixo de 22.12 falha nas validações de `engines` e pode quebrar Vite/ESLint. Atualize com nvm/nvs/Volta ou com o instalador oficial.

### Build Rust lento ou cache corrompido

```bash
cargo clean -p zuno-server   # remove só os artefatos deste pacote
cargo build -p zuno-server
```

Evite apagar `target/` inteiro sem necessidade em máquinas lentas; em último caso, `cargo clean` ainda é seguro (não afeta código nem dados). Antivírus no Windows pode deixar o build bem mais lento — considere excluir `target/` da varredura em tempo real.

### `undefined reference` / erro de compilação do SQLite (Linux)

Falta compilador C ou `pkg-config` (`build-essential`). O SQLite vem embutido; não instale `libsqlite3-dev` esperando resolver.

### Erro ao aplicar migrations

- O binário embute `server/migrations`. Se você editou uma migration já aplicada, o `sqlx` acusa checksum divergente. Em desenvolvimento, apague o banco (seção 7); nunca edite migrations já publicadas.
- Espaço em disco cheio também aparece como falha de migration.

### O app Tauri não compila

Esperado no M0: `apps/desktop/src-tauri` está fora do workspace Rust e não é testado no CI. Requer WebView2 (Windows) ou GTK/WebKitGTK (Linux) e a Tauri CLI 2. Veja `apps/desktop/README.md`.

## 9. Antes de abrir um PR

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run format:check
npm run typecheck
npm run lint
npm test
npm run build
```

Ou tudo de uma vez:

```bash
./scripts/check.sh          # Linux/macOS/bash no Windows
```

```powershell
powershell -File scripts/check.ps1   # Windows PowerShell
```

Detalhes de contribuição: [`CONTRIBUTING.md`](../../CONTRIBUTING.md). Segurança: [`SECURITY.md`](../../SECURITY.md).
