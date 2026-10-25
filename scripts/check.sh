#!/usr/bin/env bash
# Verificação local completa do Zuno (Rust + frontend).
# Uso, a partir da raiz do repositório: ./scripts/check.sh
# Não instala nada: as ferramentas precisam estar disponíveis (cargo, npm).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${ROOT_DIR}"

step() {
  printf '\n\033[1m==> %s\033[0m\n' "$1"
}

fail() {
  printf '\n\033[31mFALHOU: %s\033[0m\n' "$1" >&2
  exit 1
}

step "Verificando formatação Rust (cargo fmt --all --check)"
cargo fmt --all --check || fail "cargo fmt encontrou diferenças. Rode 'cargo fmt --all'."

step "Rodando clippy (cargo clippy --workspace --all-targets -- -D warnings)"
cargo clippy --workspace --all-targets -- -D warnings || fail "clippy reportou avisos/erros."

step "Rodando testes Rust (cargo test --workspace)"
cargo test --workspace || fail "testes Rust falharam."

step "Verificando formatação do frontend (npm run format:check)"
npm run format:check || fail "Prettier encontrou diferenças. Rode 'npm run format'."

step "Rodando typecheck (npm run typecheck)"
npm run typecheck || fail "typecheck do TypeScript falhou."

step "Rodando ESLint (npm run lint)"
npm run lint || fail "ESLint encontrou problemas."

step "Rodando testes do frontend (npm test)"
npm test || fail "testes do frontend falharam."

step "Compilando o frontend (npm run build)"
npm run build || fail "build do frontend falhou."

printf '\n\033[32mTodas as verificações passaram.\033[0m\n'
