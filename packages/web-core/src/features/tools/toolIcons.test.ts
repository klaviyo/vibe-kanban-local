import { describe, expect, it } from 'vitest';
import { ICON_ALIASES, iconComponentName } from './toolIcons';

const csrFiles = import.meta.glob(
  '../../../node_modules/@phosphor-icons/react/dist/csr/*.es.js'
);
const available = new Set(
  Object.keys(csrFiles).map((p) => p.split('/').pop()!.replace('.es.js', ''))
);

describe('iconComponentName', () => {
  it('converts kebab-case names and applies aliases', () => {
    expect(iconComponentName('robot')).toBe('Robot');
    expect(iconComponentName('magnifying-glass')).toBe('MagnifyingGlass');
    expect(iconComponentName('Git-Branch')).toBe('GitBranch');
    expect(iconComponentName('search')).toBe('MagnifyingGlass');
  });

  it('resolves every alias and the original curated names to a real icon', () => {
    const originalNames = [
      'arrows-clockwise',
      'bell',
      'bookmark',
      'brain',
      'bug',
      'calendar',
      'chat',
      'checks',
      'clipboard',
      'cloud',
      'code',
      'compass',
      'database',
      'download',
      'envelope',
      'eye',
      'fire',
      'flag',
      'flask',
      'folder',
      'gear',
      'git-branch',
      'git-pull-request',
      'globe',
      'hammer',
      'heart',
      'key',
      'lifebuoy',
      'lightbulb',
      'lightning',
      'link',
      'list',
      'lock',
      'magic-wand',
      'search',
      'megaphone',
      'note',
      'package',
      'paper-plane',
      'pencil',
      'play',
      'plug',
      'puzzle',
      'robot',
      'rocket',
      'shield',
      'sparkle',
      'star',
      'stop',
      'tag',
      'target',
      'terminal',
      'timer',
      'trash',
      'upload',
      'users',
      'wrench',
    ];
    for (const name of [...Object.keys(ICON_ALIASES), ...originalNames]) {
      expect(available, name).toContain(iconComponentName(name));
    }
  });

  it('finds the whole Phosphor set', () => {
    expect(available.size).toBeGreaterThan(1400);
  });
});
