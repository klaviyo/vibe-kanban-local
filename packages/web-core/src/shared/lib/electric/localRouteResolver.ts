import type { MutationDefinition, ShapeDefinition } from 'shared/remote-types';

export interface LocalShapeRoute {
  /** Path under `/api/remote/*` (host scoping is applied by `makeLocalApiRequest`). */
  path: string;
  /** Query-param keys to forward from the shape's params object. */
  query: readonly string[];
}

interface LocalRouteVariant {
  path: string;
  /**
   * Params required on the shape for this variant to match. The first variant
   * whose `expectedParams` are all declared by the shape wins.
   */
  expectedParams: readonly string[];
}

// Shape-table -> ordered list of local /api/remote/* descriptors. Multiple
// variants per table support shapes that share a row type but different
// scoping params (e.g., `workspaces` keyed by project_id vs owner_user_id).
// The matching local route handlers in crates/server/src/routes/remote/*.rs
// dispatch on which query param is present.
const LOCAL_ROUTES_BY_TABLE: Record<string, readonly LocalRouteVariant[]> = {
  projects: [
    { path: '/api/remote/projects', expectedParams: ['organization_id'] },
  ],
  issues: [{ path: '/api/remote/issues', expectedParams: ['project_id'] }],
  project_statuses: [
    { path: '/api/remote/project-statuses', expectedParams: ['project_id'] },
  ],
  tags: [{ path: '/api/remote/tags', expectedParams: ['project_id'] }],
  issue_assignees: [
    { path: '/api/remote/issue-assignees', expectedParams: ['project_id'] },
    { path: '/api/remote/issue-assignees', expectedParams: ['issue_id'] },
  ],
  issue_tags: [
    { path: '/api/remote/issue-tags', expectedParams: ['project_id'] },
    { path: '/api/remote/issue-tags', expectedParams: ['issue_id'] },
  ],
  issue_relationships: [
    { path: '/api/remote/issue-relationships', expectedParams: ['project_id'] },
    { path: '/api/remote/issue-relationships', expectedParams: ['issue_id'] },
  ],
  pull_requests: [
    { path: '/api/remote/pull-requests', expectedParams: ['project_id'] },
    { path: '/api/remote/pull-requests', expectedParams: ['issue_id'] },
  ],
  pull_request_issues: [
    { path: '/api/remote/pull-request-issues', expectedParams: ['project_id'] },
  ],
  workspaces: [
    { path: '/api/remote/workspaces', expectedParams: ['project_id'] },
    { path: '/api/remote/workspaces', expectedParams: ['owner_user_id'] },
  ],
  notifications: [
    { path: '/api/remote/notifications', expectedParams: ['user_id'] },
  ],
  organization_member_metadata: [
    {
      path: '/api/remote/organization-member-metadata',
      expectedParams: ['organization_id'],
    },
  ],
  users: [{ path: '/api/remote/users', expectedParams: ['organization_id'] }],
  issue_followers: [
    { path: '/api/remote/issue-followers', expectedParams: ['project_id'] },
    { path: '/api/remote/issue-followers', expectedParams: ['issue_id'] },
  ],
  issue_comments: [
    { path: '/api/remote/issue-comments', expectedParams: ['issue_id'] },
  ],
  issue_comment_reactions: [
    {
      path: '/api/remote/issue-comment-reactions',
      expectedParams: ['issue_id'],
    },
  ],
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
  const variants = LOCAL_ROUTES_BY_TABLE[shape.table];
  if (!variants) return null;
  for (const variant of variants) {
    if (variant.expectedParams.every((key) => shape.params.includes(key))) {
      return { path: variant.path, query: variant.expectedParams };
    }
  }
  return null;
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

export interface LocalMutationRoute {
  /** Base local path (no trailing slash, no id). */
  path: string;
}

// Mutation URL -> local /api/remote/* base path. Mutations whose remote URL
// is not listed here continue to go through `makeRequest()` until matching
// local routes land. POST hits the base path; PATCH/DELETE append `/{id}`;
// bulk update appends `/bulk`. The handlers in crates/server/src/routes/remote
// own the supported (POST, PATCH, DELETE) verbs for each path; transports
// that don't yet exist locally are intentionally not mapped here so the
// caller falls back to the cloud client rather than mixing transports for a
// single feature.
const LOCAL_MUTATION_ROUTE_BY_URL: Record<string, LocalMutationRoute> = {
  '/v1/issues': { path: '/api/remote/issues' },
  '/v1/issue_assignees': { path: '/api/remote/issue-assignees' },
  '/v1/issue_tags': { path: '/api/remote/issue-tags' },
  '/v1/issue_relationships': { path: '/api/remote/issue-relationships' },
  '/v1/issue_followers': { path: '/api/remote/issue-followers' },
  '/v1/issue_comments': { path: '/api/remote/issue-comments' },
  '/v1/issue_comment_reactions': {
    path: '/api/remote/issue-comment-reactions',
  },
  '/v1/projects': { path: '/api/remote/projects' },
  '/v1/project_statuses': { path: '/api/remote/project-statuses' },
  '/v1/tags': { path: '/api/remote/tags' },
  '/v1/notifications': { path: '/api/remote/notifications' },
  '/v1/pull_request_issues': { path: '/api/remote/pull-request-issues' },
};

/**
 * Resolve the local `/api/remote/*` base path for a mutation, if one exists.
 *
 * Returns `null` when the mutation has no local counterpart, signalling that
 * the caller should fall back to the remote `mutation.url` path through the
 * cloud client.
 */
export function resolveLocalMutationRoute(
  mutation: MutationDefinition<unknown, unknown, unknown>
): LocalMutationRoute | null {
  return LOCAL_MUTATION_ROUTE_BY_URL[mutation.url] ?? null;
}
