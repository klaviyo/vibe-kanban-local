import { describe, expect, it } from 'vitest';
import { matchesTagFilter } from './useKanbanFilters';

describe('matchesTagFilter', () => {
  it('any: matches when the issue has at least one selected tag', () => {
    expect(matchesTagFilter(['a'], ['a', 'b'], 'any')).toBe(true);
    expect(matchesTagFilter(['c'], ['a', 'b'], 'any')).toBe(false);
  });

  it('all: requires every selected tag', () => {
    expect(matchesTagFilter(['a', 'b', 'c'], ['a', 'b'], 'all')).toBe(true);
    expect(matchesTagFilter(['a'], ['a', 'b'], 'all')).toBe(false);
    expect(matchesTagFilter([], ['a', 'b'], 'all')).toBe(false);
  });
});
