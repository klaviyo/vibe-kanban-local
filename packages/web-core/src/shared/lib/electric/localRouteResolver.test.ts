import { describe, expect, it } from 'vitest';
import type { MutationDefinition, ShapeDefinition } from 'shared/remote-types';
import {
  buildLocalShapePath,
  resolveLocalMutationRoute,
  resolveLocalShapeRoute,
} from './localRouteResolver';

function defineShape(
  table: string,
  params: readonly string[]
): ShapeDefinition<unknown> {
  return {
    table,
    params,
    url: '/v1/shape/test',
    fallbackUrl: '/v1/fallback/test',
  } as ShapeDefinition<unknown>;
}

function defineMutation(
  url: string
): MutationDefinition<unknown, unknown, unknown> {
  return { name: 'Test', url } as MutationDefinition<unknown, unknown, unknown>;
}

describe('resolveLocalShapeRoute', () => {
  it('maps the project-keyed issues shape to /api/remote/issues', () => {
    const route = resolveLocalShapeRoute(defineShape('issues', ['project_id']));
    expect(route).toEqual({
      path: '/api/remote/issues',
      query: ['project_id'],
    });
  });

  it('maps the organization-keyed projects shape to /api/remote/projects', () => {
    const route = resolveLocalShapeRoute(
      defineShape('projects', ['organization_id'])
    );
    expect(route).toEqual({
      path: '/api/remote/projects',
      query: ['organization_id'],
    });
  });

  it('maps project_statuses to /api/remote/project-statuses (kebab-case)', () => {
    const route = resolveLocalShapeRoute(
      defineShape('project_statuses', ['project_id'])
    );
    expect(route?.path).toBe('/api/remote/project-statuses');
  });

  it('maps tags to /api/remote/tags', () => {
    const route = resolveLocalShapeRoute(defineShape('tags', ['project_id']));
    expect(route?.path).toBe('/api/remote/tags');
  });

  it('returns null for shapes without a local counterpart', () => {
    // `unknown_table` has no entry in LOCAL_ROUTES_BY_TABLE — must fall
    // through to the remote fallback URL so the cutover doesn't 404.
    expect(
      resolveLocalShapeRoute(defineShape('unknown_table', ['user_id']))
    ).toBeNull();
  });

  it('returns null when the shape params do not match the local route shape', () => {
    // Defensive: if a different shape tabled "issues" appeared with a
    // non-project_id key, we must not silently route it to the local route.
    expect(
      resolveLocalShapeRoute(defineShape('issues', ['user_id']))
    ).toBeNull();
  });

  it('maps project-keyed kanban-board shapes to /api/remote/* counterparts', () => {
    const cases: Array<[string, string]> = [
      ['issue_assignees', '/api/remote/issue-assignees'],
      ['issue_tags', '/api/remote/issue-tags'],
      ['issue_relationships', '/api/remote/issue-relationships'],
      ['pull_requests', '/api/remote/pull-requests'],
      ['pull_request_issues', '/api/remote/pull-request-issues'],
    ];
    for (const [table, path] of cases) {
      expect(
        resolveLocalShapeRoute(defineShape(table, ['project_id']))?.path
      ).toBe(path);
    }
  });

  it('maps both project- and user-keyed workspaces to /api/remote/workspaces', () => {
    const projectRoute = resolveLocalShapeRoute(
      defineShape('workspaces', ['project_id'])
    );
    expect(projectRoute).toEqual({
      path: '/api/remote/workspaces',
      query: ['project_id'],
    });
    const userRoute = resolveLocalShapeRoute(
      defineShape('workspaces', ['owner_user_id'])
    );
    expect(userRoute).toEqual({
      path: '/api/remote/workspaces',
      query: ['owner_user_id'],
    });
  });

  it('falls back to the issue-keyed local variant when no project_id is on the shape', () => {
    const route = resolveLocalShapeRoute(
      defineShape('issue_assignees', ['issue_id'])
    );
    expect(route).toEqual({
      path: '/api/remote/issue-assignees',
      query: ['issue_id'],
    });
  });

  it('maps the user-keyed notifications shape to /api/remote/notifications', () => {
    const route = resolveLocalShapeRoute(
      defineShape('notifications', ['user_id'])
    );
    expect(route).toEqual({
      path: '/api/remote/notifications',
      query: ['user_id'],
    });
  });

  it('maps organization_member_metadata to /api/remote/organization-member-metadata (kebab-case)', () => {
    const route = resolveLocalShapeRoute(
      defineShape('organization_member_metadata', ['organization_id'])
    );
    expect(route).toEqual({
      path: '/api/remote/organization-member-metadata',
      query: ['organization_id'],
    });
  });

  it('maps the org-keyed users shape to /api/remote/users', () => {
    const route = resolveLocalShapeRoute(
      defineShape('users', ['organization_id'])
    );
    expect(route).toEqual({
      path: '/api/remote/users',
      query: ['organization_id'],
    });
  });

  it('maps both project- and issue-keyed issue_followers to /api/remote/issue-followers', () => {
    const projectRoute = resolveLocalShapeRoute(
      defineShape('issue_followers', ['project_id'])
    );
    expect(projectRoute).toEqual({
      path: '/api/remote/issue-followers',
      query: ['project_id'],
    });
    const issueRoute = resolveLocalShapeRoute(
      defineShape('issue_followers', ['issue_id'])
    );
    expect(issueRoute).toEqual({
      path: '/api/remote/issue-followers',
      query: ['issue_id'],
    });
  });

  it('maps the issue-keyed issue_comments shape to /api/remote/issue-comments', () => {
    const route = resolveLocalShapeRoute(
      defineShape('issue_comments', ['issue_id'])
    );
    expect(route).toEqual({
      path: '/api/remote/issue-comments',
      query: ['issue_id'],
    });
  });

  it('maps issue_comment_reactions to /api/remote/issue-comment-reactions (kebab-case)', () => {
    const route = resolveLocalShapeRoute(
      defineShape('issue_comment_reactions', ['issue_id'])
    );
    expect(route).toEqual({
      path: '/api/remote/issue-comment-reactions',
      query: ['issue_id'],
    });
  });
});

describe('resolveLocalMutationRoute', () => {
  it('maps the issue/assignee/tag/relationship mutations to /api/remote/* paths', () => {
    expect(resolveLocalMutationRoute(defineMutation('/v1/issues'))).toEqual({
      path: '/api/remote/issues',
    });
    expect(
      resolveLocalMutationRoute(defineMutation('/v1/issue_assignees'))
    ).toEqual({ path: '/api/remote/issue-assignees' });
    expect(resolveLocalMutationRoute(defineMutation('/v1/issue_tags'))).toEqual(
      { path: '/api/remote/issue-tags' }
    );
    expect(
      resolveLocalMutationRoute(defineMutation('/v1/issue_relationships'))
    ).toEqual({ path: '/api/remote/issue-relationships' });
  });

  it('returns null for mutations without a local counterpart', () => {
    expect(
      resolveLocalMutationRoute(defineMutation('/v1/projects'))
    ).toBeNull();
    expect(
      resolveLocalMutationRoute(defineMutation('/v1/issue_comments'))
    ).toBeNull();
    expect(
      resolveLocalMutationRoute(defineMutation('/v1/pull_request_issues'))
    ).toBeNull();
  });
});

describe('buildLocalShapePath', () => {
  it('encodes the declared query params from the shape params object', () => {
    const path = buildLocalShapePath(
      { path: '/api/remote/issues', query: ['project_id'] },
      { project_id: 'p1' }
    );
    expect(path).toBe('/api/remote/issues?project_id=p1');
  });

  it('skips empty/missing param values', () => {
    const path = buildLocalShapePath(
      { path: '/api/remote/projects', query: ['organization_id'] },
      { organization_id: '' }
    );
    expect(path).toBe('/api/remote/projects');
  });

  it('only forwards declared query params, not arbitrary extras', () => {
    const path = buildLocalShapePath(
      { path: '/api/remote/issues', query: ['project_id'] },
      { project_id: 'p1', stowaway: 'no' }
    );
    expect(path).toBe('/api/remote/issues?project_id=p1');
  });
});
