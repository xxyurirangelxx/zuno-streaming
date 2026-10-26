import { ZunoApiError } from '@zuno/api-client';
import { useState } from 'react';
import type { FormEvent } from 'react';
import { describeApiError } from '../lib/errors';
import { useSession } from './AuthProvider';

type Mode = 'login' | 'setup' | 'invite';

interface FieldProps {
  id: string;
  label: string;
  type: string;
  value: string;
  onChange: (value: string) => void;
  autoComplete: string;
  hint?: string;
  minLength?: number;
  required?: boolean;
}

function Field({
  id,
  label,
  type,
  value,
  onChange,
  autoComplete,
  hint,
  minLength,
  required = true,
}: FieldProps) {
  return (
    <div>
      <label htmlFor={id} className="block text-sm font-medium text-copy">
        {label}
      </label>
      <input
        id={id}
        name={id}
        type={type}
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
        }}
        autoComplete={autoComplete}
        minLength={minLength}
        required={required}
        aria-describedby={hint ? `${id}-hint` : undefined}
        className="mt-1.5 min-h-11 w-full rounded-xl border border-line bg-surface/80 px-3 py-2 text-sm text-copy outline-none placeholder:text-copy-muted focus-visible:border-accent-blue"
      />
      {hint && (
        <p id={`${id}-hint`} className="mt-1 text-xs text-copy-muted">
          {hint}
        </p>
      )}
    </div>
  );
}

const COPY: Record<Mode, { title: string; description: string; submit: string }> = {
  login: {
    title: 'Entrar',
    description: 'Use sua conta da instância Zuno para acessar a biblioteca.',
    submit: 'Entrar',
  },
  setup: {
    title: 'Configurar instância',
    description:
      'Crie o primeiro administrador. O código de setup aparece uma única vez no log do servidor.',
    submit: 'Criar administrador',
  },
  invite: {
    title: 'Resgatar convite',
    description: 'Use o token do convite recebido para criar sua conta.',
    submit: 'Criar conta',
  },
};

export function AuthScreen() {
  const { state, login, setupClaim, redeemInvitation } = useSession();
  const [mode, setMode] = useState<Mode>(state.kind === 'setup' ? 'setup' : 'login');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [code, setCode] = useState('');
  const [token, setToken] = useState('');
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const copy = COPY[mode];

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);

    if (mode !== 'login' && password.length < 8) {
      setError('A senha precisa ter pelo menos 8 caracteres.');
      return;
    }

    setPending(true);

    try {
      if (mode === 'login') {
        await login({ username, password });
      } else if (mode === 'setup') {
        await setupClaim({ code, username, password });
      } else {
        await redeemInvitation({ token, username, password });
      }
    } catch (cause) {
      setError(
        describeApiError(cause instanceof ZunoApiError ? cause : { code: 'unknown', status: null }),
      );
    } finally {
      setPending(false);
    }
  }

  function switchMode(next: Mode) {
    setMode(next);
    setError(null);
    setPassword('');
  }

  return (
    <div className="flex min-h-screen items-center justify-center px-4 py-10">
      <div className="w-full max-w-md">
        <div className="mb-6 flex items-center justify-center gap-3">
          <img src="/logo.png" alt="" className="h-14 w-auto max-w-40 object-contain" />
          <span className="text-3xl font-semibold tracking-tight text-copy">Zuno</span>
        </div>

        <section aria-labelledby="auth-heading" className="glass-panel rounded-zuno p-6 sm:p-8">
          <p className="text-xs font-semibold tracking-[0.18em] text-accent-cyan uppercase">
            {mode === 'setup' ? 'Primeiro acesso' : 'Sua música'}
          </p>
          <h1 id="auth-heading" className="mt-1 text-2xl font-semibold text-copy">
            {copy.title}
          </h1>
          <p className="mt-2 text-sm text-copy-muted">{copy.description}</p>

          <form
            className="mt-6 space-y-4"
            onSubmit={(event) => {
              void onSubmit(event);
            }}
          >
            {mode === 'setup' && (
              <Field
                id="setup-code"
                label="Código de setup"
                type="text"
                value={code}
                onChange={setCode}
                autoComplete="one-time-code"
                hint="Código de 8 caracteres exibido no log do servidor na primeira execução."
              />
            )}

            {mode === 'invite' && (
              <Field
                id="invite-token"
                label="Token do convite"
                type="text"
                value={token}
                onChange={setToken}
                autoComplete="off"
              />
            )}

            <Field
              id="username"
              label="Usuário"
              type="text"
              value={username}
              onChange={setUsername}
              autoComplete="username"
            />

            <Field
              id="password"
              label="Senha"
              type="password"
              value={password}
              onChange={setPassword}
              autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
              minLength={mode === 'login' ? undefined : 8}
              hint={mode === 'login' ? undefined : 'Mínimo de 8 caracteres.'}
            />

            {error && (
              <p
                role="alert"
                className="rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-copy"
              >
                {error}
              </p>
            )}

            <button
              type="submit"
              disabled={pending}
              className="primary-button inline-flex min-h-11 w-full items-center justify-center rounded-xl px-5 py-2.5 text-sm font-semibold disabled:cursor-not-allowed disabled:opacity-70"
            >
              {pending ? 'Enviando…' : copy.submit}
            </button>
          </form>

          <div className="mt-5 border-t border-line/60 pt-4 text-sm">
            {mode === 'login' ? (
              <button
                type="button"
                onClick={() => switchMode('invite')}
                className="font-medium text-accent-blue underline-offset-4 hover:underline"
              >
                Tenho um convite
              </button>
            ) : (
              <button
                type="button"
                onClick={() => switchMode('login')}
                className="font-medium text-accent-blue underline-offset-4 hover:underline"
              >
                Voltar ao login
              </button>
            )}
          </div>
        </section>

        <p className="mt-5 text-center text-xs text-copy-muted">
          Sessão protegida por cookie HttpOnly. Nenhuma senha é armazenada no navegador.
        </p>
      </div>
    </div>
  );
}
