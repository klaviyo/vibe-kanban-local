import type { ShapeDefinition } from 'shared/remote-types';

export interface LocalShapeRoute {
  /** Path under `/api/remote/*` (host scoping is applied by `makeLocalApiRequest`). */
  path: string;
  /** Query-param keys to forward from the shape's params object. */
  query: readonly string[];
}

// Shape-table -> local /api/remote/* descriptor. Only shapes with a clean
// 1:1 mapping to an existing local route are listed here; reads for other
// shapes still go through the remote fallback URL until matching local
// routes are added. Keys must match the shape's `table`, and entries must
// only declare params the matching shape exposes — see
// crates/server/src/routes/remote/*.rs for the route handlers and the
// generated definitions in shared/remote-types.ts for the shape params.
const LOCAL_ROUTE_BY_TABLE: Record<
  string,
  { path: string; expectedParams: readonly string[] }
> = {
  projects: {
    path: '/api/remote/projects',
    expectedParams: ['organization_id'],
  },
  issues: { path: '/api/remote/issues', expectedParams: ['project_id'] },
  project_statuses: {
    path: '/api/remote/project-statuses',
    expectedParams: ['project_id'],
  },
  tags: { path: '/api/remote/tags', expectedParams: ['project_id'] },
};

/**
 * Resolve the local `/api/remote/*` route for a shape, if one exists.
 *
 * Returns `null` when the shape has no local counterpart, signalling that the
 * caller should fall back to the remote `fallbackUrl` path.
 */
export function resolveLocalShapeRoute(
  shape: ShapeDefinition<unknown>
): LocalShapeRoute | null {
  const entry = LOCAL_ROUTE_BY_TABLE[shape.table];
  if (!entry) return null;
  // Guard against shape/route param drift: every expected param must be
  // declared on the shape (e.g., the project-keyed PROJECT_ISSUES_SHAPE,
  // not a hypothetical user-keyed `issues` shape).
  for (const key of entry.expectedParams) {
    if (!shape.params.includes(key)) return null;
  }
  return { path: entry.path, query: entry.expectedParams };
}

/**
 * Build a fully-qualified local route path with query parameters from the
 * shape's params object. Skips unset params.
 */
export function buildLocalShapePath(
  route: LocalShapeRoute,
  params: Record<string, string>
): string {
  const search = new URLSearchParams();
  for (const key of route.query) {
    const value = params[key];
    if (value) search.set(key, value);
  }
  const qs = search.toString();
  return qs ? `${route.path}?${qs}` : route.path;
}
