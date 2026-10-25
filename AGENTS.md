# Zuno — Agent Instructions

## Project context

Zuno is an open-source, self-hosted music streaming platform.

Read these documents before making changes:

1. PRD.md — Product requirements and architecture.
2. PLANO\_AGENTE.md — Implementation plan and acceptance criteria.

## Source of truth

- The PRD defines product requirements.
- The implementation plan defines milestones and task order.
- Do not silently invent decisions for unresolved requirements.
- Document important architectural decisions as ADRs.
- Keep documentation synchronized with the implementation.

## Technology

- Backend: Rust, Axum, Tokio.
- Database: SQLite.
- Frontend: React, TypeScript, Vite, Tailwind.
- Desktop and Android: Tauri 2.
- Target server: Raspberry Pi 3, with Linux and Windows support.

## Development rules

- Follow the implementation plan incrementally.
- Prefer a modular monolith.
- Optimize for limited memory and CPU.
- Do not require paid APIs.
- Never commit secrets or credentials.
- Add automated tests for new functionality.
- Run relevant checks before marking tasks complete.
- Never delete user music without explicit authorization.
- Treat external integrations as optional.
- Preserve backward compatibility where practical.

## Workflow

1. Read the project documentation.
2. Identify the next incomplete milestone.
3. Propose a short implementation plan.
4. Implement one coherent task at a time.
5. Run tests and static checks.
6. Report files changed, results, and remaining issues.
7. Update documentation and task status.

## Restrictions

- Do not implement the entire project in a single unreviewed change.
- Do not assume that unverified third-party APIs are available.
- Do not expose the server publicly by default.
- Do not execute destructive operations without explicit approval.
- Do not choose a final open-source license without approval.
