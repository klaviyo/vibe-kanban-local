import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ShapeDefinition } from 'shared/remote-types';
import { fetchShapeRows } from './fetchShape';

vi.mock('@/shared/lib/remoteApi', () => ({
  makeRequest: vi.fn(),
}));
vi.mock('@/shared/lib/localApiTransport', () => ({
  makeLocalApiRequest: vi.fn(),
}));

import { makeRequest } from '@/shared/lib/remoteApi';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';

const mockedMakeRequest = vi.mocked(makeRequest);
const mockedMakeLocalApiRequest = vi.mocked(makeLocalApiRequest);

function jsonResponse(body: unknown, init: ResponseInit = { status: 200 }) {
  return new Response(JSON.stringify(body), {
    ...init,
    headers: { 'Content-Type': 'application/json', ...(init.headers ?? {}) },
  });
}

function defineShape<T>(
  table: string,
  params: readonly string[],
  fallbackUrl: string
): ShapeDefinition<T> {
  return {
    table,
    params,
    url: '/v1/shape/test',
    fallbackUrl,
  } as ShapeDefinition<T>;
}

describe('fetchShapeRows', () => {
  beforeEach(() => {
    mockedMakeRequest.mockReset();
    mockedMakeLocalApiRequest.mockReset();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('routes mapped shapes through the local transport with the /api/remote/* path', async () => {
    mockedMakeLocalApiRequest.mockResolvedValueOnce(
      jsonResponse({
        success: true,
        data: { issues: [{ id: 'i1' }, { id: 'i2' }] },
        error_data: null,
        message: null,
      })
    );
    const shape = defineShape<{ id: string }>(
      'issues',
      ['project_id'] as const,
      '/v1/fallback/issues'
    );

    const rows = await fetchShapeRows(shape, { project_id: 'p1' });

    expect(rows).toEqual([{ id: 'i1' }, { id: 'i2' }]);
    expect(mockedMakeLocalApiRequest).toHaveBeenCalledTimes(1);
    expect(mockedMakeRequest).not.toHaveBeenCalled();
    const [path, init] = mockedMakeLocalApiRequest.mock.calls[0];
    expect(path).toBe('/api/remote/issues?project_id=p1');
    expect(init?.method).toBe('GET');
  });

  it('routes the org-keyed projects shape through the local /api/remote/projects route', async () => {
    mockedMakeLocalApiRequest.mockResolvedValueOnce(
      jsonResponse({ success: true, data: { projects: [{ id: 'pr1' }] } })
    );
    const shape = defineShape<{ id: string }>(
      'projects',
      ['organization_id'] as const,
      '/v1/fallback/projects'
    );

    await fetchShapeRows(shape, { organization_id: 'o1' });

    const [path] = mockedMakeLocalApiRequest.mock.calls[0];
    expect(path).toBe('/api/remote/projects?organization_id=o1');
  });

  it('falls back to the remote fallback URL via makeRequest when no local route is mapped', async () => {
    // Use a synthetic table that has no entry in LOCAL_ROUTES_BY_TABLE so
    // the fallback path is exercised independent of which tables happen to
    // be locally routed at any given point in the cutover.
    mockedMakeRequest.mockResolvedValueOnce(
      jsonResponse({
        unknown_table: [{ id: 'x1' }],
      })
    );
    const shape = defineShape<{ id: string }>(
      'unknown_table',
      ['user_id'] as const,
      '/v1/fallback/unknown_table'
    );

    const rows = await fetchShapeRows(shape, { user_id: 'u1' });

    expect(rows).toEqual([{ id: 'x1' }]);
    expect(mockedMakeLocalApiRequest).not.toHaveBeenCalled();
    expect(mockedMakeRequest).toHaveBeenCalledTimes(1);
    const [path] = mockedMakeRequest.mock.calls[0];
    expect(path).toBe('/v1/fallback/unknown_table?user_id=u1');
  });

  it('throws when the response is not ok', async () => {
    mockedMakeLocalApiRequest.mockResolvedValueOnce(
      new Response('boom', { status: 500 })
    );
    const shape = defineShape<{ id: string }>(
      'issues',
      ['project_id'] as const,
      '/v1/fallback/issues'
    );

    await expect(fetchShapeRows(shape, { project_id: 'p1' })).rejects.toThrow(
      /HTTP 500/
    );
  });

  it('throws when the table key is missing in the response payload', async () => {
    mockedMakeLocalApiRequest.mockResolvedValueOnce(
      jsonResponse({ success: true, data: {} })
    );
    const shape = defineShape<{ id: string }>(
      'issues',
      ['project_id'] as const,
      '/v1/fallback/issues'
    );

    await expect(fetchShapeRows(shape, { project_id: 'p1' })).rejects.toThrow(
      /missing "issues" array/
    );
  });
});
