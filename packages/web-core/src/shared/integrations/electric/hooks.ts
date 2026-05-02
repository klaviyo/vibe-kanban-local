import { useCallback, useEffect, useMemo, useRef } from 'react';
import {
  useMutation,
  useQuery,
  useQueryClient,
  type QueryKey,
} from '@tanstack/react-query';

import { makeRequest } from '@/shared/lib/remoteApi';
import { useSyncErrorContext } from '@/shared/hooks/useSyncErrorContext';
import type { MutationDefinition, ShapeDefinition } from 'shared/remote-types';
import type {
  InsertResult,
  MutationResult,
  SyncError,
} from '@/shared/lib/electric/types';

// Type helpers for extracting types from MutationDefinition
type MutationCreateType<M> =
  M extends MutationDefinition<unknown, infer C, unknown> ? C : never;
type MutationUpdateType<M> =
  M extends MutationDefinition<unknown, unknown, infer U> ? U : never;

/**
 * Base result type returned by useShape (read-only).
 */
export interface UseShapeResult<TRow> {
  /** The current data array */
  data: TRow[];
  /** Whether the initial fetch is still in flight */
  isLoading: boolean;
  /** Error from the most recent fetch, if any */
  error: SyncError | null;
  /** Function to retry after an error */
  retry: () => void;
}

/**
 * Extended result when mutation is provided — adds insert/update/remove.
 */
export interface UseShapeMutationResult<TRow, TCreate, TUpdate>
  extends UseShapeResult<TRow> {
  /** Insert a new row (optimistic), returns row and persistence promise */
  insert: (data: TCreate) => InsertResult<TRow>;
  /** Update a row by ID (optimistic), returns persistence promise */
  update: (id: string, changes: Partial<TUpdate>) => MutationResult;
  /** Update multiple rows in a single optimistic transaction */
  updateMany: (
    updates: Array<{ id: string; changes: Partial<TUpdate> }>
  ) => MutationResult;
  /** Delete a row by ID (optimistic), returns persistence promise */
  remove: (id: string) => MutationResult;
}

/**
 * Options for the useShape hook.
 */
export interface UseShapeOptions<
  M extends
    | MutationDefinition<unknown, unknown, unknown>
    | undefined = undefined,
> {
  /**
   * Whether to enable the underlying query.
   * When false, returns empty data and no-op mutation functions.
   * @default true
   */
  enabled?: boolean;
  /**
   * Optional mutation definition. When provided, the hook returns
   * insert/update/remove functions for optimistic mutations.
   */
  mutation?: M;
}

type Row = Record<string, unknown> & { id?: string };

const DEFAULT_STALE_TIME_MS = 30 * 1000;

const queryKeyFor = (
  table: string,
  params: Record<string, string>
): QueryKey => ['shape', table, params];

function buildFallbackPath(
  fallbackUrl: string,
  params: Record<string, string>
): string {
  let path = fallbackUrl;
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (!value) continue;
    path = path.replace(`{${key}}`, encodeURIComponent(value));
    query.set(key, value);
  }
  const queryString = query.toString();
  return queryString ? `${path}?${queryString}` : path;
}

async function parseErrorMessage(
  response: Response,
  fallback: string
): Promise<string> {
  try {
    const body = (await response.json()) as {
      message?: string;
      error?: string;
    };
    return body.message || body.error || fallback;
  } catch {
    return fallback;
  }
}

function toSyncError(error: unknown): SyncError {
  if (error && typeof error === 'object' && 'message' in error) {
    const e = error as { message?: unknown; status?: unknown };
    return {
      message: typeof e.message === 'string' ? e.message : 'Request failed',
      status: typeof e.status === 'number' ? e.status : undefined,
    };
  }
  return { message: typeof error === 'string' ? error : 'Request failed' };
}

class HttpError extends Error {
  constructor(
    public readonly status: number,
    message: string
  ) {
    super(message);
    this.name = 'HttpError';
  }
}

async function fetchShapeData<T extends Row>(
  shape: ShapeDefinition<T>,
  params: Record<string, string>
): Promise<T[]> {
  const path = buildFallbackPath(shape.fallbackUrl, params);
  const response = await makeRequest(path, {
    method: 'GET',
    cache: 'no-store',
  });

  if (!response.ok) {
    const message = await parseErrorMessage(
      response,
      `Failed to fetch ${shape.table}`
    );
    throw new HttpError(response.status, message);
  }

  const payload = (await response.json()) as Record<string, unknown>;
  const rows = payload[shape.table];
  if (!Array.isArray(rows)) {
    throw new Error(`Response missing "${shape.table}" array`);
  }
  return rows as T[];
}

/**
 * Hook for subscribing to a remote collection's data via React Query,
 * with optional optimistic mutation support.
 *
 * Backed by HTTP calls to the same endpoints used by the previous Electric
 * fallback path; mutations go through the standard mutation URL with
 * optimistic cache updates and invalidate-on-settle.
 *
 * @example
 * // Read-only:
 * const { data, isLoading } = useShape(PROJECT_PULL_REQUESTS_SHAPE, { project_id });
 *
 * // With mutations:
 * const { data, insert, update, remove } = useShape(
 *   PROJECT_ISSUES_SHAPE,
 *   { project_id },
 *   { mutation: ISSUE_MUTATION }
 * );
 */
export function useShape<
  T extends Record<string, unknown>,
  M extends
    | MutationDefinition<unknown, unknown, unknown>
    | undefined = undefined,
>(
  shape: ShapeDefinition<T>,
  params: Record<string, string>,
  options: UseShapeOptions<M> = {} as UseShapeOptions<M>
): M extends MutationDefinition<unknown, unknown, unknown>
  ? UseShapeMutationResult<T, MutationCreateType<M>, MutationUpdateType<M>>
  : UseShapeResult<T> {
  const { enabled = true, mutation } = options;

  const queryClient = useQueryClient();

  // Stabilize params reference so React Query's queryKey identity is stable
  // when callers pass a freshly-built object each render.
  const paramsKey = JSON.stringify(params);
  const stableParams = useMemo(
    () => JSON.parse(paramsKey) as Record<string, string>,
    [paramsKey]
  );

  const queryKey = useMemo(
    () => queryKeyFor(shape.table, stableParams),
    [shape.table, stableParams]
  );

  const query = useQuery<T[], Error>({
    queryKey,
    queryFn: () => fetchShapeData(shape, stableParams),
    enabled,
    staleTime: DEFAULT_STALE_TIME_MS,
  });

  const items = enabled ? (query.data ?? []) : [];
  const isLoading = enabled ? query.isLoading : false;

  const error: SyncError | null = useMemo(() => {
    if (!enabled || !query.error) return null;
    return toSyncError(query.error);
  }, [enabled, query.error]);

  const retry = useCallback(() => {
    void query.refetch();
  }, [query]);

  // Sync the latest items into a ref so optimistic insert can reconcile
  // server-assigned fields when the persisted snapshot differs by id.
  const itemsRef = useRef<T[]>(items);
  useEffect(() => {
    itemsRef.current = items;
  }, [items]);

  // Surface fetch errors through the global SyncErrorContext just like the
  // legacy Electric implementation did, so the navbar banner still works.
  const syncErrorContext = useSyncErrorContext();
  const registerErrorFn = syncErrorContext?.registerError;
  const clearErrorFn = syncErrorContext?.clearError;
  const streamId = useMemo(
    () => `${shape.table}:${paramsKey}`,
    [shape.table, paramsKey]
  );

  useEffect(() => {
    if (error && registerErrorFn) {
      registerErrorFn(streamId, shape.table, error, retry);
    } else if (!error && clearErrorFn) {
      clearErrorFn(streamId);
    }
    return () => {
      clearErrorFn?.(streamId);
    };
  }, [error, streamId, shape.table, retry, registerErrorFn, clearErrorFn]);

  // --- Mutations ---------------------------------------------------------

  const mutationUrl = mutation?.url;
  const mutationName = mutation?.name;

  const writeOptimistic = useCallback(
    (updater: (rows: T[]) => T[]): T[] | undefined => {
      const previous = queryClient.getQueryData<T[]>(queryKey);
      queryClient.setQueryData<T[]>(queryKey, (current) =>
        updater(current ?? [])
      );
      return previous;
    },
    [queryClient, queryKey]
  );

  const insertMutation = useMutation<
    T,
    Error,
    Row,
    { previous: T[] | undefined; optimisticId: string }
  >({
    mutationFn: async (row: Row): Promise<T> => {
      if (!mutationUrl) {
        throw new Error(
          'insert called without a mutation definition on useShape'
        );
      }
      const response = await makeRequest(mutationUrl, {
        method: 'POST',
        body: JSON.stringify(row),
      });
      if (!response.ok) {
        const message = await parseErrorMessage(
          response,
          `Failed to create ${mutationName ?? shape.table}`
        );
        throw new HttpError(response.status, message);
      }
      const payload = (await response.json()) as { data: T } | T;
      const created = (payload as { data?: T }).data ?? (payload as T);
      return created;
    },
    onMutate: async (row) => {
      await queryClient.cancelQueries({ queryKey });
      const optimisticId = String(row.id);
      const previous = writeOptimistic((rows) => [...rows, row as T]);
      return { previous, optimisticId };
    },
    onError: (_err, _row, context) => {
      if (context?.previous !== undefined) {
        queryClient.setQueryData<T[]>(queryKey, context.previous);
      }
    },
    onSuccess: (created, _row, context) => {
      if (!context) return;
      const optimisticId = context.optimisticId;
      const createdId =
        typeof (created as Row).id === 'string'
          ? String((created as Row).id)
          : optimisticId;
      queryClient.setQueryData<T[]>(queryKey, (current) => {
        const rows = current ?? [];
        const without = rows.filter(
          (item) => String((item as Row).id ?? '') !== optimisticId
        );
        return [
          ...without.filter(
            (item) => String((item as Row).id ?? '') !== createdId
          ),
          created,
        ];
      });
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey });
    },
  });

  const updateOneMutation = useMutation<
    T | null,
    Error,
    { id: string; changes: Record<string, unknown> },
    { previous: T[] | undefined }
  >({
    mutationFn: async ({ id, changes }): Promise<T | null> => {
      if (!mutationUrl) {
        throw new Error(
          'update called without a mutation definition on useShape'
        );
      }
      const response = await makeRequest(`${mutationUrl}/${id}`, {
        method: 'PATCH',
        body: JSON.stringify(changes),
      });
      if (!response.ok) {
        const message = await parseErrorMessage(
          response,
          `Failed to update ${mutationName ?? shape.table}`
        );
        throw new HttpError(response.status, message);
      }
      try {
        const payload = (await response.json()) as { data?: T } | T;
        return (payload as { data?: T }).data ?? (payload as T);
      } catch {
        return null;
      }
    },
    onMutate: async ({ id, changes }) => {
      await queryClient.cancelQueries({ queryKey });
      const previous = writeOptimistic((rows) =>
        rows.map((item) =>
          String((item as Row).id ?? '') === id
            ? ({ ...(item as Row), ...changes } as T)
            : item
        )
      );
      return { previous };
    },
    onError: (_err, _vars, context) => {
      if (context?.previous !== undefined) {
        queryClient.setQueryData<T[]>(queryKey, context.previous);
      }
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey });
    },
  });

  const updateManyMutation = useMutation<
    void,
    Error,
    Array<{ id: string; changes: Record<string, unknown> }>,
    { previous: T[] | undefined }
  >({
    mutationFn: async (updates): Promise<void> => {
      if (!mutationUrl) {
        throw new Error(
          'updateMany called without a mutation definition on useShape'
        );
      }
      if (updates.length === 0) return;
      const body = updates.map((u) => ({ id: u.id, ...u.changes }));
      const response = await makeRequest(`${mutationUrl}/bulk`, {
        method: 'POST',
        body: JSON.stringify({ updates: body }),
      });
      if (!response.ok) {
        const message = await parseErrorMessage(
          response,
          `Failed to bulk update ${mutationName ?? shape.table}`
        );
        throw new HttpError(response.status, message);
      }
    },
    onMutate: async (updates) => {
      await queryClient.cancelQueries({ queryKey });
      const changesById = new Map(updates.map((u) => [u.id, u.changes]));
      const previous = writeOptimistic((rows) =>
        rows.map((item) => {
          const id = String((item as Row).id ?? '');
          const changes = changesById.get(id);
          if (!changes) return item;
          return { ...(item as Row), ...changes } as T;
        })
      );
      return { previous };
    },
    onError: (_err, _updates, context) => {
      if (context?.previous !== undefined) {
        queryClient.setQueryData<T[]>(queryKey, context.previous);
      }
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey });
    },
  });

  const removeMutation = useMutation<
    void,
    Error,
    string,
    { previous: T[] | undefined }
  >({
    mutationFn: async (id) => {
      if (!mutationUrl) {
        throw new Error(
          'remove called without a mutation definition on useShape'
        );
      }
      const response = await makeRequest(`${mutationUrl}/${id}`, {
        method: 'DELETE',
      });
      if (!response.ok) {
        const message = await parseErrorMessage(
          response,
          `Failed to delete ${mutationName ?? shape.table}`
        );
        throw new HttpError(response.status, message);
      }
    },
    onMutate: async (id) => {
      await queryClient.cancelQueries({ queryKey });
      const previous = writeOptimistic((rows) =>
        rows.filter((item) => String((item as Row).id ?? '') !== id)
      );
      return { previous };
    },
    onError: (_err, _id, context) => {
      if (context?.previous !== undefined) {
        queryClient.setQueryData<T[]>(queryKey, context.previous);
      }
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey });
    },
  });

  const insert = useCallback(
    (insertData: unknown): InsertResult<T> => {
      const dataWithId: Row = {
        id: crypto.randomUUID(),
        ...(insertData as Record<string, unknown>),
      };
      if (!mutationUrl) {
        return {
          data: dataWithId as unknown as T,
          persisted: Promise.resolve(dataWithId as unknown as T),
        };
      }
      const persisted = insertMutation
        .mutateAsync(dataWithId)
        .then((created) => {
          // Prefer the most recent synced row (matches by id) so callers see
          // any server-assigned fields after the cache is refreshed.
          const id = String(dataWithId.id);
          const synced = itemsRef.current.find(
            (item) => String((item as Row).id ?? '') === id
          );
          return synced ?? created ?? (dataWithId as unknown as T);
        });
      return {
        data: dataWithId as unknown as T,
        persisted,
      };
    },
    [insertMutation, mutationUrl]
  );

  const update = useCallback(
    (id: string, changes: unknown): MutationResult => {
      if (!mutationUrl) {
        return { persisted: Promise.resolve() };
      }
      const persisted = updateOneMutation
        .mutateAsync({ id, changes: changes as Record<string, unknown> })
        .then(() => undefined);
      return { persisted };
    },
    [updateOneMutation, mutationUrl]
  );

  const updateMany = useCallback(
    (updates: Array<{ id: string; changes: unknown }>): MutationResult => {
      if (!mutationUrl || updates.length === 0) {
        return { persisted: Promise.resolve() };
      }
      const persisted = updateManyMutation
        .mutateAsync(
          updates.map((u) => ({
            id: u.id,
            changes: u.changes as Record<string, unknown>,
          }))
        )
        .then(() => undefined);
      return { persisted };
    },
    [updateManyMutation, mutationUrl]
  );

  const remove = useCallback(
    (id: string): MutationResult => {
      if (!mutationUrl) {
        return { persisted: Promise.resolve() };
      }
      const persisted = removeMutation.mutateAsync(id).then(() => undefined);
      return { persisted };
    },
    [removeMutation, mutationUrl]
  );

  const base: UseShapeResult<T> = {
    data: items,
    isLoading,
    error,
    retry,
  };

  if (mutation) {
    return {
      ...base,
      insert,
      update,
      updateMany,
      remove,
    } as M extends MutationDefinition<unknown, unknown, unknown>
      ? UseShapeMutationResult<T, MutationCreateType<M>, MutationUpdateType<M>>
      : UseShapeResult<T>;
  }

  return base as M extends MutationDefinition<unknown, unknown, unknown>
    ? UseShapeMutationResult<T, MutationCreateType<M>, MutationUpdateType<M>>
    : UseShapeResult<T>;
}
