import { makeRequest } from '@/shared/lib/remoteApi';
import type { ShapeDefinition } from 'shared/remote-types';

function buildPath(
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
  const response = await makeRequest(buildPath(shape.fallbackUrl, params), {
    method: 'GET',
    cache: 'no-store',
  });
  if (!response.ok) {
    throw new Error(`Failed to fetch ${shape.table}: HTTP ${response.status}`);
  }
  const payload = (await response.json()) as Record<string, unknown>;
  const rows = payload[shape.table];
  if (!Array.isArray(rows)) {
    throw new Error(`Response missing "${shape.table}" array`);
  }
  return rows as T[];
}
