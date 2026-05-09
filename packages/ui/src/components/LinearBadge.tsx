import { cn } from '../lib/cn';
import { LinkSimpleIcon } from '@phosphor-icons/react';

/**
 * Parses the ticket id (e.g. "SMS2-700") out of a Linear issue URL
 * (`https://linear.app/<org>/issue/<TEAM-NUM>/<slug>`). Returns null when
 * the URL doesn't match the expected pattern — caller decides whether to
 * skip the badge or fall back to a generic label.
 */
export function parseLinearTicketId(url: string): string | null {
  const match = url.match(
    /^https?:\/\/linear\.app\/[^/]+\/issue\/([A-Z][A-Z0-9]*-\d+)(?:\/[^?#]*)?/i
  );
  return match ? match[1].toUpperCase() : null;
}

export interface LinearBadgeProps {
  url: string;
  className?: string;
}

/**
 * Badge linking out to a Linear ticket. Shape mirrors `PrBadge`: a
 * compact, clickable pill with an icon + the ticket id. Click stops
 * propagation so it doesn't toggle whatever card/row owns it.
 */
export function LinearBadge({ url, className }: LinearBadgeProps) {
  const ticketId = parseLinearTicketId(url);
  return (
    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      onClick={(e) => e.stopPropagation()}
      className={cn(
        'flex items-center gap-half px-1.5 py-0.5 rounded text-xs font-medium transition-colors',
        'bg-secondary text-normal hover:bg-secondary/80',
        className
      )}
      title={ticketId ? `Linear: ${ticketId}` : `Linear: ${url}`}
    >
      <LinkSimpleIcon className="size-icon-2xs" weight="bold" />
      <span>{ticketId ?? 'Linear'}</span>
    </a>
  );
}
