// Tests for the useShape mutation contract — focused on the optimistic
// insert/update/remove cache transitions and rollback-on-error semantics
// the hook composes via @tanstack/react-query. The hook itself can't be
// rendered here without a DOM/test-library install, so these tests
// exercise the same QueryClient primitives the hook drives so future
// regressions to those contracts get caught.

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClient } from '@tanstack/react-query';

type Row = { id: string; title?: string };

function makeClient() {
  return new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
}

const QUERY_KEY = ['shape', 'issues', { project_id: 'p1' }] as const;

interface OptimisticInsertContext {
  previous: Row[] | undefined;
  optimisticId: string;
}

async function runOptimisticInsert(
  client: QueryClient,
  newRow: Row,
  mutationFn: (row: Row) => Promise<Row>
) {
  await client.cancelQueries({ queryKey: QUERY_KEY });
  const previous = client.getQueryData<Row[]>(QUERY_KEY);
  client.setQueryData<Row[]>(QUERY_KEY, (current) => [
    ...(current ?? []),
    newRow,
  ]);
  const ctx: OptimisticInsertContext = {
    previous,
    optimisticId: newRow.id,
  };
  try {
    const created = await mutationFn(newRow);
    client.setQueryData<Row[]>(QUERY_KEY, (current) => {
      const rows = current ?? [];
      const without = rows.filter((r) => r.id !== ctx.optimisticId);
      return [...without.filter((r) => r.id !== created.id), created];
    });
    return created;
  } catch (err) {
    if (ctx.previous !== undefined) {
      client.setQueryData<Row[]>(QUERY_KEY, ctx.previous);
    }
    throw err;
  }
}

async function runOptimisticUpdate(
  client: QueryClient,
  id: string,
  changes: Partial<Row>,
  mutationFn: () => Promise<void>
) {
  await client.cancelQueries({ queryKey: QUERY_KEY });
  const previous = client.getQueryData<Row[]>(QUERY_KEY);
  client.setQueryData<Row[]>(QUERY_KEY, (current) =>
    (current ?? []).map((r) => (r.id === id ? { ...r, ...changes } : r))
  );
  try {
    await mutationFn();
  } catch (err) {
    if (previous !== undefined) {
      client.setQueryData<Row[]>(QUERY_KEY, previous);
    }
    throw err;
  }
}

async function runOptimisticRemove(
  client: QueryClient,
  id: string,
  mutationFn: () => Promise<void>
) {
  await client.cancelQueries({ queryKey: QUERY_KEY });
  const previous = client.getQueryData<Row[]>(QUERY_KEY);
  client.setQueryData<Row[]>(QUERY_KEY, (current) =>
    (current ?? []).filter((r) => r.id !== id)
  );
  try {
    await mutationFn();
  } catch (err) {
    if (previous !== undefined) {
      client.setQueryData<Row[]>(QUERY_KEY, previous);
    }
    throw err;
  }
}

describe('useShape optimistic mutation contract', () => {
  let client: QueryClient;

  beforeEach(() => {
    client = makeClient();
    client.setQueryData<Row[]>(QUERY_KEY, [
      { id: 'a', title: 'A' },
      { id: 'b', title: 'B' },
    ]);
  });

  afterEach(() => {
    client.clear();
    vi.restoreAllMocks();
  });

  describe('insert', () => {
    it('writes the optimistic row immediately and reconciles to the server-assigned record on success', async () => {
      const created: Row = { id: 'srv-c', title: 'C' };
      const mutationFn = vi.fn().mockResolvedValue(created);

      const promise = runOptimisticInsert(
        client,
        { id: 'tmp-c', title: 'C' },
        mutationFn
      );

      // Optimistic write is visible while the mutation is in flight.
      const optimistic = client.getQueryData<Row[]>(QUERY_KEY);
      expect(optimistic?.map((r) => r.id)).toContain('tmp-c');

      await promise;

      const final = client.getQueryData<Row[]>(QUERY_KEY) ?? [];
      // Optimistic id is replaced by the server-assigned record.
      expect(final.map((r) => r.id)).not.toContain('tmp-c');
      expect(final.map((r) => r.id)).toContain('srv-c');
      expect(mutationFn).toHaveBeenCalledTimes(1);
    });

    it('rolls back to the pre-mutation snapshot when the mutation fails', async () => {
      const mutationFn = vi.fn().mockRejectedValue(new Error('insert failed'));

      await expect(
        runOptimisticInsert(client, { id: 'tmp-c', title: 'C' }, mutationFn)
      ).rejects.toThrow('insert failed');

      const final = client.getQueryData<Row[]>(QUERY_KEY) ?? [];
      expect(final.map((r) => r.id)).toEqual(['a', 'b']);
    });
  });

  describe('update', () => {
    it('applies the patch optimistically and persists when the mutation resolves', async () => {
      const mutationFn = vi.fn().mockResolvedValue(undefined);

      await runOptimisticUpdate(client, 'a', { title: 'A2' }, mutationFn);

      const final = client.getQueryData<Row[]>(QUERY_KEY) ?? [];
      expect(final.find((r) => r.id === 'a')?.title).toBe('A2');
    });

    it('rolls the row back to the previous title when the mutation rejects', async () => {
      const mutationFn = vi.fn().mockRejectedValue(new Error('update failed'));

      await expect(
        runOptimisticUpdate(client, 'a', { title: 'A2' }, mutationFn)
      ).rejects.toThrow('update failed');

      const final = client.getQueryData<Row[]>(QUERY_KEY) ?? [];
      expect(final.find((r) => r.id === 'a')?.title).toBe('A');
    });
  });

  describe('remove', () => {
    it('drops the row optimistically and keeps it dropped on success', async () => {
      const mutationFn = vi.fn().mockResolvedValue(undefined);

      await runOptimisticRemove(client, 'a', mutationFn);

      const final = client.getQueryData<Row[]>(QUERY_KEY) ?? [];
      expect(final.map((r) => r.id)).toEqual(['b']);
    });

    it('restores the row when the delete mutation rejects', async () => {
      const mutationFn = vi.fn().mockRejectedValue(new Error('delete failed'));

      await expect(
        runOptimisticRemove(client, 'a', mutationFn)
      ).rejects.toThrow('delete failed');

      const final = client.getQueryData<Row[]>(QUERY_KEY) ?? [];
      expect(final.map((r) => r.id)).toEqual(['a', 'b']);
    });
  });
});
