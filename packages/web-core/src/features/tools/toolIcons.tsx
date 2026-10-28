import { useEffect, useState, type ComponentType, type ReactNode } from 'react';
import { WrenchIcon } from '@phosphor-icons/react';

type IconComponent = ComponentType<{ className?: string; weight?: 'bold' }>;
type IconModule = Record<string, IconComponent>;

// One lazy chunk per Phosphor icon, fetched only when a tool names it, so the
// full set costs nothing in the main bundle.
const ICON_LOADERS = import.meta.glob<IconModule>(
  '../../../node_modules/@phosphor-icons/react/dist/csr/*.es.js'
);

// Short names kept for configs written against the original curated set.
export const ICON_ALIASES: Record<string, string> = {
  chat: 'ChatCircle',
  download: 'DownloadSimple',
  'paper-plane': 'PaperPlaneTilt',
  puzzle: 'PuzzlePiece',
  rocket: 'RocketLaunch',
  search: 'MagnifyingGlass',
  shield: 'ShieldCheck',
  terminal: 'TerminalWindow',
  upload: 'UploadSimple',
};

/** `magnifying-glass` -> `MagnifyingGlass`, applying aliases. */
export function iconComponentName(name: string): string {
  const key = name.trim().toLowerCase();
  return (
    ICON_ALIASES[key] ??
    key
      .split('-')
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join('')
  );
}

const isIconName = (name: string) => /^[a-z0-9-]+$/i.test(name.trim());

const iconCache = new Map<string, Promise<IconComponent | null>>();

function loadIcon(name: string): Promise<IconComponent | null> {
  const componentName = iconComponentName(name);
  let promise = iconCache.get(componentName);
  if (!promise) {
    const load = Object.entries(ICON_LOADERS).find(([path]) =>
      path.endsWith(`/csr/${componentName}.es.js`)
    )?.[1];
    promise = load
      ? load().then(
          (mod) => mod[`${componentName}Icon`] ?? mod[componentName] ?? null
        )
      : Promise.resolve(null);
    iconCache.set(componentName, promise);
  }
  return promise;
}

function ToolIcon({ name, className }: { name: string; className: string }) {
  // undefined = still loading, null = unknown name
  const [Icon, setIcon] = useState<IconComponent | null | undefined>();

  useEffect(() => {
    let current = true;
    setIcon(undefined);
    void loadIcon(name).then((component) => {
      if (current) setIcon(() => component);
    });
    return () => {
      current = false;
    };
  }, [name]);

  if (Icon === undefined) return <span className={className} aria-hidden />;
  if (Icon === null) return <WrenchIcon className={className} weight="bold" />;
  return <Icon className={className} weight="bold" />;
}

/**
 * Icon for a tool. `name` is a Phosphor icon name in kebab-case (as on
 * phosphoricons.com) or one of the short aliases; anything else renders as
 * text, so an emoji works. A missing icon is a wrench.
 */
export function renderToolIcon(
  name: string | null,
  className = 'size-icon-base'
): ReactNode {
  if (!name) return <WrenchIcon className={className} weight="bold" />;
  if (isIconName(name)) return <ToolIcon name={name} className={className} />;
  return <span className="text-base leading-none">{name}</span>;
}
