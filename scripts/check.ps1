# Verificação local completa do Zuno (Rust + frontend).
# Uso, a partir da raiz do repositório: pwsh -File scripts/check.ps1
# Não instala nada: as ferramentas precisam estar disponíveis (cargo, npm).
$ErrorActionPreference = 'Stop'

$RootDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $RootDir

function Invoke-Step {
    param(
        [Parameter(Mandatory = $true)][string]$Title,
        [Parameter(Mandatory = $true)][scriptblock]$Action,
        [Parameter(Mandatory = $true)][string]$FailureMessage
    )

    Write-Host ""
    Write-Host "==> $Title" -ForegroundColor Cyan
    & $Action
    if ($LASTEXITCODE -ne 0) {
        Write-Host ""
        Write-Host "FALHOU: $FailureMessage" -ForegroundColor Red
        exit 1
    }
}

Invoke-Step -Title "Verificando formatação Rust (cargo fmt --all --check)" -Action {
    cargo fmt --all --check
} -FailureMessage "cargo fmt encontrou diferenças. Rode 'cargo fmt --all'."

Invoke-Step -Title "Rodando clippy (cargo clippy --workspace --all-targets -- -D warnings)" -Action {
    cargo clippy --workspace --all-targets -- -D warnings
} -FailureMessage "clippy reportou avisos/erros."

Invoke-Step -Title "Rodando testes Rust (cargo test --workspace)" -Action {
    cargo test --workspace
} -FailureMessage "testes Rust falharam."

Invoke-Step -Title "Verificando formatação do frontend (npm run format:check)" -Action {
    npm run format:check
} -FailureMessage "Prettier encontrou diferenças. Rode 'npm run format'."

Invoke-Step -Title "Rodando typecheck (npm run typecheck)" -Action {
    npm run typecheck
} -FailureMessage "typecheck do TypeScript falhou."

Invoke-Step -Title "Rodando ESLint (npm run lint)" -Action {
    npm run lint
} -FailureMessage "ESLint encontrou problemas."

Invoke-Step -Title "Rodando testes do frontend (npm test)" -Action {
    npm test
} -FailureMessage "testes do frontend falharam."

Invoke-Step -Title "Compilando o frontend (npm run build)" -Action {
    npm run build
} -FailureMessage "build do frontend falhou."

Write-Host ""
Write-Host "Todas as verificações passaram." -ForegroundColor Green
