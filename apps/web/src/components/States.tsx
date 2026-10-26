import type { ReactNode } from 'react';

export function LoadingSkeleton({
  rows = 6,
  label = 'Carregando…',
}: {
  rows?: number;
  label?: string;
}) {
  return (
    <div role="status" aria-live="polite" aria-busy="true" className="space-y-3">
      <span className="sr-only">{label}</span>
      {Array.from({ length: rows }, (_, index) => (
        <div
          key={index}
          aria-hidden="true"
          className="flex items-center gap-4 rounded-xl border border-line/50 bg-surface/40 p-3"
        >
          <div className="h-12 w-12 shrink-0 rounded-lg bg-surface-muted motion-safe:animate-pulse" />
          <div className="min-w-0 flex-1 space-y-2">
            <div className="h-2.5 w-2/5 rounded-full bg-surface-muted motion-safe:animate-pulse" />
            <div className="h-2 w-1/4 rounded-full bg-surface-muted motion-safe:animate-pulse" />
          </div>
        </div>
      ))}
    </div>
  );
}

interface EmptyStateProps {
  title: string;
  description: string;
  children?: ReactNode;
}

export function EmptyState({ title, description, children }: EmptyStateProps) {
  return (
    <div className="rounded-zuno border border-dashed border-line/80 bg-surface/40 p-8 text-center">
      <p className="text-lg font-semibold text-copy">{title}</p>
      <p className="mx-auto mt-2 max-w-prose text-sm text-copy-muted">{description}</p>
      {children && <div className="mt-5 flex justify-center gap-3">{children}</div>}
    </div>
  );
}

interface ErrorStateProps {
  title?: string;
  message: string;
  onRetry?: () => void;
}

export function ErrorState({
  title = 'Não foi possível carregar',
  message,
  onRetry,
}: ErrorStateProps) {
  return (
    <div role="alert" className="rounded-zuno border border-red-500/40 bg-red-500/5 p-6 text-left">
      <p className="text-lg font-semibold text-copy">{title}</p>
      <p className="mt-2 max-w-prose text-sm text-copy-muted">{message}</p>
      {onRetry && (
        <button
          type="button"
          onClick={onRetry}
          className="primary-button mt-4 inline-flex min-h-11 items-center rounded-xl px-4 py-2 text-sm font-semibold"
        >
          Tentar novamente
        </button>
      )}
    </div>
  );
}
