import { ZunoApiError } from '@zuno/api-client';
import type { UploadRequestOptions, ZunoClient } from '@zuno/api-client';
import type { UploadCompleteResponse, UploadCreated, User } from '@zuno/shared-types';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { AuthProvider } from '../app/AuthProvider';
import { UploadView } from './UploadView';

const user: User = { id: 'u1', username: 'ana', role: 'user' };

const created: UploadCreated = {
  upload_id: 'up1',
  original_filename: 'faixa.opus',
  size_bytes: 10,
  sha256: 'abc',
  status: 'staged',
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

interface MockUploadClient {
  client: ZunoClient;
  createUpload: ReturnType<typeof vi.fn>;
  getUpload: ReturnType<typeof vi.fn>;
  completeUpload: ReturnType<typeof vi.fn>;
  deleteUpload: ReturnType<typeof vi.fn>;
}

function createMockClient(): MockUploadClient {
  const createUpload = vi.fn();
  const getUpload = vi.fn();
  const completeUpload = vi.fn();
  const deleteUpload = vi.fn().mockResolvedValue(undefined);

  const client = {
    me: vi.fn().mockResolvedValue(user),
    createUpload,
    getUpload,
    completeUpload,
    deleteUpload,
  } as unknown as ZunoClient;

  return { client, createUpload, getUpload, completeUpload, deleteUpload };
}

function renderUpload(client: ZunoClient) {
  return render(
    <AuthProvider client={client}>
      <UploadView />
    </AuthProvider>,
  );
}

function fileInput(): HTMLInputElement {
  return screen.getByLabelText(/Arraste arquivos Opus/i) as HTMLInputElement;
}

function opusFile(name: string): File {
  return new File(['conteudo-opus'], name, { type: 'audio/ogg' });
}

function selectFiles(files: File[]): void {
  fireEvent.change(fileInput(), { target: { files } });
}

describe('UploadView', () => {
  it('rejeita não-Opus no cliente antes de qualquer envio', async () => {
    const mock = createMockClient();
    renderUpload(mock.client);
    await screen.findByRole('heading', { name: 'Enviar músicas' });

    selectFiles([new File(['x'], 'musica.mp3', { type: 'audio/mpeg' })]);

    expect(await screen.findByText(/Formato não aceito/i)).toBeInTheDocument();
    expect(screen.getByText(/converta no cliente Windows\/Tauri/i)).toBeInTheDocument();
    expect(mock.createUpload).not.toHaveBeenCalled();
  });

  it('mostra progresso real e conclui em indexado', async () => {
    const mock = createMockClient();
    const upload = deferred<UploadCreated>();
    let progress: ((value: { loaded: number; total: number; percent: number }) => void) | null =
      null;

    mock.createUpload.mockImplementation((_file: File, options?: UploadRequestOptions) => {
      progress = options?.onProgress ?? null;
      return upload.promise;
    });
    mock.completeUpload.mockResolvedValue({
      upload_id: 'up1',
      status: 'indexed',
      track_id: 't1',
    } satisfies UploadCompleteResponse);

    renderUpload(mock.client);
    await screen.findByRole('heading', { name: 'Enviar músicas' });
    selectFiles([opusFile('faixa.opus')]);

    await waitFor(() => {
      expect(mock.createUpload).toHaveBeenCalledTimes(1);
    });

    act(() => {
      progress?.({
        loaded: 25,
        total: 100,
        percent: 25,
      });
    });

    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '25');

    await act(async () => {
      upload.resolve(created);
      await upload.promise;
    });

    expect(await screen.findByText('Indexado')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Ir para a biblioteca' })).toHaveAttribute(
      'href',
      '#/library/tracks',
    );
  });

  it('envia a fila em sequência, um arquivo por vez', async () => {
    const mock = createMockClient();
    const first = deferred<UploadCreated>();
    const second = deferred<UploadCreated>();

    mock.createUpload.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    mock.completeUpload.mockResolvedValue({
      upload_id: 'up1',
      status: 'indexed',
      track_id: 't1',
    } satisfies UploadCompleteResponse);

    renderUpload(mock.client);
    await screen.findByRole('heading', { name: 'Enviar músicas' });
    selectFiles([opusFile('primeira.opus'), opusFile('segunda.opus')]);

    await waitFor(() => {
      expect(mock.createUpload).toHaveBeenCalledTimes(1);
    });
    expect(mock.createUpload.mock.calls[0]?.[0]).toMatchObject({ name: 'primeira.opus' });

    await act(async () => {
      first.resolve({ ...created, upload_id: 'up1' });
      await first.promise;
    });

    await waitFor(() => {
      expect(mock.createUpload).toHaveBeenCalledTimes(2);
    });
    expect(mock.createUpload.mock.calls[1]?.[0]).toMatchObject({ name: 'segunda.opus' });

    await act(async () => {
      second.resolve({ ...created, upload_id: 'up2' });
      await second.promise;
    });

    expect(await screen.findByText('Ir para a biblioteca')).toBeInTheDocument();
  });

  it('mostra erro acionável e permite tentar novamente', async () => {
    const mock = createMockClient();
    mock.createUpload
      .mockRejectedValueOnce(
        new ZunoApiError('too_large', 'Arquivo grande demais.', 413, {
          body: { code: 'too_large', message: 'Arquivo grande demais.' },
        }),
      )
      .mockResolvedValueOnce(created);
    mock.completeUpload.mockResolvedValue({
      upload_id: 'up1',
      status: 'indexed',
      track_id: 't1',
    } satisfies UploadCompleteResponse);

    renderUpload(mock.client);
    await screen.findByRole('heading', { name: 'Enviar músicas' });
    selectFiles([opusFile('grande.opus')]);

    expect(
      await screen.findByText(/Arquivo grande demais para o limite configurado/i),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Tentar novamente' }));

    expect(await screen.findByText('Indexado')).toBeInTheDocument();
    await waitFor(() => {
      expect(mock.createUpload).toHaveBeenCalledTimes(2);
    });
  });

  it('cancela durante a validação, aborta e remove o staging', async () => {
    const mock = createMockClient();
    mock.createUpload.mockResolvedValue(created);
    mock.completeUpload.mockImplementation(
      (_id: string, signal?: AbortSignal) =>
        new Promise<UploadCompleteResponse>((_resolve, reject) => {
          signal?.addEventListener(
            'abort',
            () => reject(new ZunoApiError('canceled', 'Cancelado.', null)),
            { once: true },
          );
        }),
    );

    renderUpload(mock.client);
    await screen.findByRole('heading', { name: 'Enviar músicas' });
    selectFiles([opusFile('cancela.opus')]);

    expect(await screen.findByText('Validando')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Cancelar' }));

    expect(await screen.findByText('Cancelado')).toBeInTheDocument();
    await waitFor(() => {
      expect(mock.deleteUpload).toHaveBeenCalledWith('up1');
    });
  });

  it('exibe duplicata 409 com link para a faixa e opção de descartar', async () => {
    const mock = createMockClient();
    mock.createUpload.mockResolvedValue(created);
    mock.completeUpload.mockRejectedValue(
      new ZunoApiError('duplicate', 'Arquivo já existe.', 409, {
        body: { upload_id: 'up1', status: 'duplicate', track_id: 't9' },
      }),
    );

    renderUpload(mock.client);
    await screen.findByRole('heading', { name: 'Enviar músicas' });
    selectFiles([opusFile('duplicada.opus')]);

    const link = await screen.findByRole('link', {
      name: /Já existe na biblioteca — ver faixa \(t9\)/i,
    });

    expect(link).toHaveAttribute('href', '#/library/tracks');
    expect(screen.getByRole('button', { name: 'Descartar' })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Descartar' }));

    await waitFor(() => {
      expect(screen.queryByText('duplicada.opus')).not.toBeInTheDocument();
    });
  });
});
