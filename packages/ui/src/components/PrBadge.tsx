import { cn } from '../lib/cn';
import {
  CheckIcon,
  GitPullRequestIcon,
  SpinnerGapIcon,
  XIcon,
} from '@phosphor-icons/react';

export type PrBadgeStatus = 'open' | 'merged' | 'closed';

export type PrBadgeCiStatus = 'passing' | 'failing' | 'pending';

export interface PrBadgeProps {
  number: number;
  url: string;
  status: PrBadgeStatus;
  isDraft?: boolean;
  ciStatus?: PrBadgeCiStatus | null;
  className?: string;
}

export function PrBadge({ number, url, status, isDraft, ciStatus, className }: PrBadgeProps) {
  const isOpen = status === 'open';
  const isDraftOpen = isOpen && isDraft;
  return (
    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      onClick={(e) => e.stopPropagation()}
      className={cn(
        'flex items-center gap-half px-1.5 py-0.5 rounded text-xs font-medium transition-colors',
        status === 'merged'
          ? 'bg-merged/10 text-merged hover:bg-merged/20'
          : status === 'closed'
            ? 'bg-error/10 text-error hover:bg-error/20'
            : isDraftOpen
              ? 'bg-low/20 text-normal hover:bg-low/30'
              : 'bg-success/10 text-success hover:bg-success/20',
        className
      )}
    >
      <GitPullRequestIcon className="size-icon-2xs" weight="bold" />
      <span>#{number}</span>
      {isOpen && ciStatus === 'passing' && (
        <CheckIcon className="size-icon-2xs" weight="bold" />
      )}
      {isOpen && ciStatus === 'failing' && (
        <XIcon className="size-icon-2xs" weight="bold" />
      )}
      {isOpen && ciStatus === 'pending' && (
        <SpinnerGapIcon
          className="size-icon-2xs animate-spin"
          weight="bold"
        />
      )}
    </a>
  );
}
