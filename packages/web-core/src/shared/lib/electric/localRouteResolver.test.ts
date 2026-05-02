import { describe, expect, it } from 'vitest';
import type { ShapeDefinition } from 'shared/remote-types';
import {
  buildLocalShapePath,
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
    expect(
      resolveLocalShapeRoute(defineShape('notifications', ['user_id']))
    ).toBeNull();
    expect(
      resolveLocalShapeRoute(defineShape('issue_comments', ['issue_id']))
    ).toBeNull();
  });

  it('returns null when the shape params do not match the local route shape', () => {
    // Defensive: if a different shape tabled "issues" appeared with a
    // non-project_id key, we must not silently route it to the local route.
    expect(
      resolveLocalShapeRoute(defineShape('issues', ['user_id']))
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
