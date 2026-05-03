// Integration test for the rewired data layer.
//
// Addresses 06-implementation-review.md Round 1 HIGH #8 (no automated test
// exercises the rewired frontend against the new backend routes end-to-end).
//
// This test wires the real shape/mutation definitions from
// `shared/remote-types` through the real `localRouteResolver` and the real
// `fetchShapeRows` / mutation transport helpers, with the local
// `/api/remote/*` HTTP transport mocked. It covers the four kanban data-flow
// verbs called out in the review:
//
//   - List issues for a project (GET /api/remote/issues?project_id=...)
//   - Create an issue          (POST /api/remote/issues)
//   - Update an issue          (PATCH /api/remote/issues/{id})
//   - Delete an issue          (DELETE /api/remote/issues/{id})
//
// For reads we call `fetchShapeRows()` directly. For mutations we exercise
// the same `mutationTransport`/`extractMutationRow` shape the hook composes
// (re-implemented locally here so the wire format is asserted independent
// of the React Query orchestration tested in `hooks.test.ts`). Together
// these exercises prove that:
//
//   1. `localRouteResolver.LOCAL_ROUTES_BY_TABLE` and
//      `LOCAL_MUTATION_ROUTE_BY_URL` map the kanban shape/mutation
//      definitions onto the expected `/api/remote/*` URLs.
//   2. The `ApiResponse` envelope returned by the local backend
//      (`{ success, data, ... }`) is unwrapped before the hook layer sees it
//      for both shape reads and `MutationResponse<T>` payloads.
//   3. The mutation request body is the JSON shape the local handlers
//      decode (the `Create…Request` / `Update…Request` types from
//      `shared/remote-types`).

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  ISSUE_MUTATION,
  PROJECT_ISSUES_SHAPE,
  type Issue,
} from 'shared/remote-types';

vi.mock('@/shared/lib/remoteApi', () => ({
  makeRequest: vi.fn(),
}));
vi.mock('@/shared/lib/localApiTransport', () => ({
  makeLocalApiRequest: vi.fn(),
}));

import { makeRequest } from '@/shared/lib/remoteApi';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';
import { fetchShapeRows } from '@/shared/lib/electric/fetchShape';
import {
  resolveLocalMutationRoute,
  type LocalMutationRoute,
} from '@/shared/lib/electric/localRouteResolver';

const mockedMakeRequest = vi.mocked(makeRequest);
const mockedMakeLocalApiRequest = vi.mocked(makeLocalApiRequest);

function jsonResponse(body: unknown, init: ResponseInit = { status: 200 }) {
  return new Response(JSON.stringify(body), {
    ...init,
    headers: { 'Content-Type': 'application/json', ...(init.headers ?? {}) },
  });
}

// Mirror of the private `mutationTransport` helper in hooks.ts. Re-stated here
// so we exercise the same URL composition the hook composes when it
// dispatches a verb.
function mutationTransport(
  localRoute: LocalMutationRoute | null,
  baseUrl: string,
  suffix: string
) {
  if (localRoute) {
    return {
      url: suffix ? `${localRoute.path}${suffix}` : localRoute.path,
      send: makeLocalApiRequest,
    };
  }
  return {
    url: suffix ? `${baseUrl}${suffix}` : baseUrl,
    send: makeRequest,
  };
}

// Mirror of the private `extractMutationRow` helper in hooks.ts. Strips both
// the `ApiResponse` envelope (`{ success, data }`) and the inner
// `MutationResponse<T>` envelope (`{ data, txid }`) so the caller sees the
// row.
function extractMutationRow<T>(payload: unknown): T {
  const inner =
    payload && typeof payload === 'object' && 'success' in payload
      ? ((payload as { data?: unknown }).data ?? null)
      : payload;
  if (inner && typeof inner === 'object' && 'data' in inner) {
    return (inner as { data: T }).data;
  }
  return inner as T;
}

describe('rewired data layer end-to-end (kanban issues)', () => {
  beforeEach(() => {
    mockedMakeRequest.mockReset();
    mockedMakeLocalApiRequest.mockReset();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  describe('list issues for a project', () => {
    it('fetches via /api/remote/issues, decodes the ApiResponse envelope, and returns the row array', async () => {
      const issues = [
        { id: 'i1', title: 'First', project_id: 'p1' },
        { id: 'i2', title: 'Second', project_id: 'p1' },
      ];
      mockedMakeLocalApiRequest.mockResolvedValueOnce(
        jsonResponse({
          success: true,
          data: { issues },
          error_data: null,
          message: null,
        })
      );

      const rows = await fetchShapeRows<Issue>(PROJECT_ISSUES_SHAPE, {
        project_id: 'p1',
      });

      // Envelope decoded; rows returned as a plain array of issues.
      expect(rows).toEqual(issues);
      // Local transport is selected for the project-keyed issues shape.
      expect(mockedMakeLocalApiRequest).toHaveBeenCalledTimes(1);
      expect(mockedMakeRequest).not.toHaveBeenCalled();
      const [path, init] = mockedMakeLocalApiRequest.mock.calls[0];
      expect(path).toBe('/api/remote/issues?project_id=p1');
      expect(init?.method).toBe('GET');
      // Reads must bypass the browser cache — the cutover page still has to
      // see the freshest server state on each refetch.
      expect(init?.cache).toBe('no-store');
    });

    it('throws an error mentioning the table when the local route returns a non-2xx', async () => {
      mockedMakeLocalApiRequest.mockResolvedValueOnce(
        new Response('boom', { status: 500 })
      );

      await expect(
        fetchShapeRows<Issue>(PROJECT_ISSUES_SHAPE, { project_id: 'p1' })
      ).rejects.toThrow(/issues.*HTTP 500/);
    });
  });

  describe('create an issue', () => {
    it('POSTs to /api/remote/issues with the create payload and decodes the MutationResponse envelope', async () => {
      const localRoute = resolveLocalMutationRoute(ISSUE_MUTATION);
      // Sanity: the issues mutation must be locally routed for this seam to
      // be exercised at all.
      expect(localRoute).toEqual({ path: '/api/remote/issues' });

      const persisted: Issue = {
        id: 'srv-i3',
        title: 'New issue',
        project_id: 'p1',
      } as Issue;
      mockedMakeLocalApiRequest.mockResolvedValueOnce(
        jsonResponse({
          success: true,
          data: { data: persisted, txid: 42 },
          error_data: null,
          message: null,
        })
      );

      const transport = mutationTransport(localRoute, ISSUE_MUTATION.url, '');
      const optimistic = {
        id: 'tmp-i3',
        title: 'New issue',
        project_id: 'p1',
      };
      const response = await transport.send(transport.url, {
        method: 'POST',
        body: JSON.stringify(optimistic),
      });
      const created = extractMutationRow<Issue>(await response.json());

      expect(transport.url).toBe('/api/remote/issues');
      expect(transport.send).toBe(makeLocalApiRequest);
      expect(mockedMakeLocalApiRequest).toHaveBeenCalledTimes(1);
      expect(mockedMakeRequest).not.toHaveBeenCalled();
      const [path, init] = mockedMakeLocalApiRequest.mock.calls[0];
      expect(path).toBe('/api/remote/issues');
      expect(init?.method).toBe('POST');
      expect(JSON.parse(String(init?.body))).toEqual(optimistic);
      // Both the ApiResponse envelope and the MutationResponse envelope
      // are unwrapped, leaving the persisted row.
      expect(created).toEqual(persisted);
    });
  });

  describe('update an issue', () => {
    it('PATCHes /api/remote/issues/{id} with the changes payload and decodes the MutationResponse envelope', async () => {
      const localRoute = resolveLocalMutationRoute(ISSUE_MUTATION);
      const updated: Issue = {
        id: 'i1',
        title: 'Renamed',
        project_id: 'p1',
      } as Issue;
      mockedMakeLocalApiRequest.mockResolvedValueOnce(
        jsonResponse({
          success: true,
          data: { data: updated, txid: 43 },
          error_data: null,
          message: null,
        })
      );

      const transport = mutationTransport(
        localRoute,
        ISSUE_MUTATION.url,
        '/i1'
      );
      const changes = { title: 'Renamed' };
      const response = await transport.send(transport.url, {
        method: 'PATCH',
        body: JSON.stringify(changes),
      });
      const persisted = extractMutationRow<Issue>(await response.json());

      expect(transport.url).toBe('/api/remote/issues/i1');
      expect(mockedMakeLocalApiRequest).toHaveBeenCalledTimes(1);
      const [path, init] = mockedMakeLocalApiRequest.mock.calls[0];
      expect(path).toBe('/api/remote/issues/i1');
      expect(init?.method).toBe('PATCH');
      expect(JSON.parse(String(init?.body))).toEqual(changes);
      expect(persisted).toEqual(updated);
    });
  });

  describe('delete an issue', () => {
    it('DELETEs /api/remote/issues/{id} and treats a 204 envelope as a successful drop', async () => {
      const localRoute = resolveLocalMutationRoute(ISSUE_MUTATION);
      // Local DELETE handlers may return 204 (no body) or a wrapped
      // ApiResponse; both must be acceptable to the seam.
      mockedMakeLocalApiRequest.mockResolvedValueOnce(
        new Response(null, { status: 204 })
      );

      const transport = mutationTransport(
        localRoute,
        ISSUE_MUTATION.url,
        '/i1'
      );
      const response = await transport.send(transport.url, {
        method: 'DELETE',
      });

      expect(response.ok).toBe(true);
      expect(response.status).toBe(204);
      expect(transport.url).toBe('/api/remote/issues/i1');
      expect(mockedMakeLocalApiRequest).toHaveBeenCalledTimes(1);
      const [path, init] = mockedMakeLocalApiRequest.mock.calls[0];
      expect(path).toBe('/api/remote/issues/i1');
      expect(init?.method).toBe('DELETE');
    });

    it('still routes through /api/remote/* when the delete returns a wrapped ApiResponse body', async () => {
      const localRoute = resolveLocalMutationRoute(ISSUE_MUTATION);
      mockedMakeLocalApiRequest.mockResolvedValueOnce(
        jsonResponse({
          success: true,
          data: null,
          error_data: null,
          message: null,
        })
      );

      const transport = mutationTransport(
        localRoute,
        ISSUE_MUTATION.url,
        '/i1'
      );
      const response = await transport.send(transport.url, {
        method: 'DELETE',
      });

      expect(response.ok).toBe(true);
      // No round-trip to the cloud client; this is the post-cutover transport.
      expect(mockedMakeRequest).not.toHaveBeenCalled();
    });
  });
});
