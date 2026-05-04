import { makeRequest } from '@/shared/lib/remoteApi';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';
import type { ShapeDefinition } from 'shared/remote-types';
import {
  buildLocalShapePath,
  resolveLocalShapeRoute,
} from '@/shared/lib/electric/localRouteResolver';

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

function extractRows<T>(payload: unknown, table: string): T[] {
  // Local `/api/remote/*` routes wrap responses in an `ApiResponse` envelope
  // (`{ success, data, ... }`). Remote fallback routes return the table
  // payload directly. Strip the envelope when present, then read the rows.
  const root =
    payload && typeof payload === 'object' && 'success' in payload
      ? ((payload as { data?: unknown }).data ?? null)
      : payload;
  const rows = (root as Record<string, unknown> | null)?.[table];
  if (!Array.isArray(rows)) {
    throw new Error(`Response missing "${table}" array`);
  }
  return rows as T[];
}

/**
 * Fetch a remote shape's rows as a one-shot HTTP request.
 *
 * Used by callers that need the data outside of a React component (e.g.
 * navigation resolution) or for cross-key aggregation where the React Query
 * `useShape` hook can't be looped.
 */
export async function fetchShapeRows<T>(
  shape: ShapeDefinition<T>,
  params: Record<string, string>
): Promise<T[]> {
  const localRoute = resolveLocalShapeRoute(shape);
  const response = localRoute
    ? await makeLocalApiRequest(buildLocalShapePath(localRoute, params), {
        method: 'GET',
        cache: 'no-store',
      })
    : await makeRequest(buildFallbackPath(shape.fallbackUrl, params), {
        method: 'GET',
        cache: 'no-store',
      });
  if (!response.ok) {
    throw new Error(`Failed to fetch ${shape.table}: HTTP ${response.status}`);
  }
  const payload = (await response.json()) as unknown;
  return extractRows<T>(payload, shape.table);
}
