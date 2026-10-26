import { ZunoApiError } from '@zuno/api-client';
import type { ZunoClient } from '@zuno/api-client';
import { useCallback, useEffect, useRef, useState } from 'react';
import type { UploadProgress } from '@zuno/shared-types';

export type UploadItemStatus =
  'queued' | 'sending' | 'validating' | 'indexed' | 'duplicate' | 'error' | 'canceled';

export interface UploadQueueItem {
  id: string;
  file: File;
  fileName: string;
  sizeBytes: number;
  status: UploadItemStatus;
  progress: number;
  uploadId: string | null;
  trackId: string | null;
  error: ZunoApiError | null;
}

export interface RejectedFile {
  id: string;
  fileName: string;
  reason: string;
}

export interface UploadQueueOptions {
  client: ZunoClient;
  onIndexed: () => void;
  onUnauthorized: () => void;
}

export interface UploadQueue {
  items: UploadQueueItem[];
  rejected: RejectedFile[];
  addFiles: (files: FileList | File[]) => void;
  cancel: (id: string) => void;
  retry: (id: string) => void;
  discard: (id: string) => void;
  clearCompleted: () => void;
  dismissRejected: (id: string) => void;
}

const OPUS_EXTENSIONS = ['.opus', '.ogg'];

function fileExtension(name: string): string {
  const dot = name.lastIndexOf('.');
  return dot === -1 ? '' : name.slice(dot).toLowerCase();
}

function isOpusFileName(name: string): boolean {
  return OPUS_EXTENSIONS.includes(fileExtension(name));
}

function toApiError(error: unknown): ZunoApiError {
  if (error instanceof ZunoApiError) {
    return error;
  }

  return new ZunoApiError('unknown', 'Ocorreu uma falha inesperada.', null);
}

function readTrackId(body: unknown): string | null {
  if (typeof body !== 'object' || body === null) {
    return null;
  }

  const trackId = (body as Record<string, unknown>).track_id;
  return typeof trackId === 'string' ? trackId : null;
}

export function useUploadQueue({ client, onIndexed, onUnauthorized }: UploadQueueOptions) {
  const [items, setItems] = useState<UploadQueueItem[]>([]);
  const [rejected, setRejected] = useState<RejectedFile[]>([]);
  const itemsRef = useRef<UploadQueueItem[]>([]);
  const processingRef = useRef(false);
  const controllersRef = useRef(new Map<string, AbortController>());
  const nextIdRef = useRef(0);
  const pumpRef = useRef<() => void>(() => undefined);

  const commit = useCallback((updater: (current: UploadQueueItem[]) => UploadQueueItem[]) => {
    const next = updater(itemsRef.current);
    itemsRef.current = next;
    setItems(next);
  }, []);

  const updateItem = useCallback(
    (id: string, patch: Partial<UploadQueueItem>) => {
      commit((current) => current.map((item) => (item.id === id ? { ...item, ...patch } : item)));
    },
    [commit],
  );

  const runItem = useCallback(
    async (item: UploadQueueItem) => {
      const controller = new AbortController();
      controllersRef.current.set(item.id, controller);
      updateItem(item.id, { status: 'sending', progress: 0, error: null });
      let uploadId: string | null = null;

      try {
        const created = await client.createUpload(item.file, {
          signal: controller.signal,
          onProgress: (progress: UploadProgress) => {
            updateItem(item.id, { progress: progress.percent });
          },
        });
        uploadId = created.upload_id;
        updateItem(item.id, { status: 'validating', uploadId });

        if (controller.signal.aborted) {
          throw new ZunoApiError('canceled', 'A operação foi cancelada.', null);
        }

        const completed = await client.completeUpload(uploadId, controller.signal);
        updateItem(item.id, {
          status: 'indexed',
          trackId: completed.track_id,
          progress: 100,
          uploadId,
        });
        onIndexed();
      } catch (cause) {
        const error = toApiError(cause);
        const trackId = readTrackId(error.body);

        if (error.status === 409) {
          updateItem(item.id, {
            status: 'duplicate',
            trackId,
            uploadId,
            progress: 100,
          });
          return;
        }

        if (controller.signal.aborted || error.code === 'canceled') {
          if (uploadId) {
            void client.deleteUpload(uploadId).catch(() => {
              // O staging pode já ter sido removido; o estado local já é cancelled.
            });
          }

          updateItem(item.id, { status: 'canceled', uploadId, trackId: null });
          return;
        }

        if (error.status === 401) {
          onUnauthorized();
        }

        updateItem(item.id, { status: 'error', error, uploadId });
      } finally {
        controllersRef.current.delete(item.id);
      }
    },
    [client, onIndexed, onUnauthorized, updateItem],
  );

  // Fila sequencial: apenas um item é processado por vez, sem saturar o Pi.
  const pump = useCallback(() => {
    if (processingRef.current) {
      return;
    }

    const next = itemsRef.current.find((item) => item.status === 'queued');

    if (!next) {
      return;
    }

    processingRef.current = true;

    void runItem(next).finally(() => {
      processingRef.current = false;
      pumpRef.current();
    });
  }, [runItem]);

  useEffect(() => {
    pumpRef.current = pump;
  }, [pump]);

  const addFiles = useCallback(
    (files: FileList | File[]) => {
      const incoming = Array.from(files);

      if (incoming.length === 0) {
        return;
      }

      const accepted: UploadQueueItem[] = [];
      const refused: RejectedFile[] = [];

      for (const file of incoming) {
        if (!isOpusFileName(file.name)) {
          refused.push({
            id: `rejected-${nextIdRef.current++}`,
            fileName: file.name,
            reason:
              'Formato não aceito. A biblioteca é Opus; converta no cliente Windows/Tauri (M4) — o navegador não converte.',
          });
          continue;
        }

        accepted.push({
          id: `upload-${nextIdRef.current++}`,
          file,
          fileName: file.name,
          sizeBytes: file.size,
          status: 'queued',
          progress: 0,
          uploadId: null,
          trackId: null,
          error: null,
        });
      }

      if (refused.length > 0) {
        setRejected((current) => [...current, ...refused]);
      }

      if (accepted.length > 0) {
        commit((current) => [...current, ...accepted]);
        pumpRef.current();
      }
    },
    [commit],
  );

  const cancel = useCallback(
    (id: string) => {
      const controller = controllersRef.current.get(id);

      if (controller) {
        controller.abort();
        return;
      }

      commit((current) =>
        current.map((item) =>
          item.id === id && item.status === 'queued'
            ? { ...item, status: 'canceled', progress: 0 }
            : item,
        ),
      );
    },
    [commit],
  );

  const retry = useCallback(
    (id: string) => {
      commit((current) =>
        current.map((item) =>
          item.id === id && (item.status === 'error' || item.status === 'canceled')
            ? {
                ...item,
                status: 'queued',
                progress: 0,
                uploadId: null,
                trackId: null,
                error: null,
              }
            : item,
        ),
      );
      pumpRef.current();
    },
    [commit],
  );

  const discard = useCallback(
    (id: string) => {
      commit((current) => current.filter((item) => item.id !== id));
    },
    [commit],
  );

  const clearCompleted = useCallback(() => {
    commit((current) =>
      current.filter(
        (item) =>
          item.status !== 'indexed' && item.status !== 'duplicate' && item.status !== 'canceled',
      ),
    );
  }, [commit]);

  const dismissRejected = useCallback((id: string) => {
    setRejected((current) => current.filter((item) => item.id !== id));
  }, []);

  return {
    items,
    rejected,
    addFiles,
    cancel,
    retry,
    discard,
    clearCompleted,
    dismissRejected,
  } satisfies UploadQueue;
}
