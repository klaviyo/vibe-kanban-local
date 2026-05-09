'use client';

import type { MouseEvent } from 'react';
import { cn } from '../lib/cn';
import { Draggable } from '@hello-pangea/dnd';
import {
  CircleIcon,
  DotsSixVerticalIcon,
  HandIcon,
  TriangleIcon,
} from '@phosphor-icons/react';
import { PriorityIcon, type PriorityLevel } from './PriorityIcon';
import { StatusDot } from './StatusDot';
import { RunningDots } from './RunningDots';
import { KanbanBadge } from './KanbanBadge';
import { KanbanAssignee, type KanbanAssigneeUser } from './KanbanAssignee';
import { PrBadge } from './PrBadge';
import { LinearBadge } from './LinearBadge';
import type { KanbanPullRequest } from './KanbanCardContent';
import {
  RelationshipBadge,
  type RelationshipDisplayType,
} from './RelationshipBadge';
import { Checkbox } from './Checkbox';

/**
 * Aggregate workspace state for the issue, ordered by precedence (highest
 * urgency first). When the issue has multiple workspaces, the worst-case
 * signal wins so a row scanned in the kanban list view always communicates
 * the most actionable state. `null` means no signal — fall back to the
 * column's `StatusDot` color.
 */
export type WorkspaceActivitySignal =
  | 'pendingApproval'
  | 'running'
  | 'failed'
  | 'unseenActivity';

/**
 * Formats a date as a relative time string (e.g., "1d", "2h", "3m")
 */
function formatRelativeTime(dateString: string): string {
  const date = new Date(dateString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMinutes = Math.floor(diffMs / (1000 * 60));
  const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays > 0) {
    return `${diffDays}d`;
  }
  if (diffHours > 0) {
    return `${diffHours}h`;
  }
  if (diffMinutes > 0) {
    return `${diffMinutes}m`;
  }
  return 'now';
}

const MAX_VISIBLE_TAGS = 2;
const MAX_VISIBLE_PRS = 2;

export interface IssueListRowIssue {
  id: string;
  simple_id: string;
  title: string;
  priority: PriorityLevel | null;
  created_at: string;
}

export interface IssueListRowTag {
  id: string;
  name: string;
  color: string;
}

export interface IssueListRowRelationship {
  relationshipId: string;
  displayType: RelationshipDisplayType;
  relatedIssueDisplayId: string;
}

export interface IssueListRowProps {
  issue: IssueListRowIssue;
  index: number;
  statusColor: string;
  tags: IssueListRowTag[];
  relationships?: IssueListRowRelationship[];
  assignees: KanbanAssigneeUser[];
  /**
   * Pull requests linked to this issue, rendered as `PrBadge`s in the
   * row's right-side group (matching the kanban card view's badges).
   * The first `MAX_VISIBLE_PRS` are shown; the remainder collapses to
   * "+N".
   */
  pullRequests?: KanbanPullRequest[];
  /**
   * Linear ticket URL pulled off `issue.extension_metadata.linear_url`.
   * When set, renders a `LinearBadge` next to the PR badges.
   */
  linearUrl?: string | null;
  /**
   * Highest-urgency workspace signal for this issue. When set, replaces the
   * (column-redundant) `StatusDot` with the matching activity glyph.
   */
  workspaceActivity?: WorkspaceActivitySignal | null;
  onClick: (e: MouseEvent) => void;
  isSelected: boolean;
  isMultiSelectActive?: boolean;
  isChecked?: boolean;
  onCheckboxChange?: (checked: boolean) => void;
  className?: string;
}

export function IssueListRow({
  issue,
  index,
  statusColor,
  tags,
  relationships = [],
  assignees,
  pullRequests = [],
  linearUrl,
  workspaceActivity,
  onClick,
  isSelected,
  isMultiSelectActive = false,
  isChecked = false,
  onCheckboxChange,
  className,
}: IssueListRowProps) {
  const showCheckbox = isMultiSelectActive || isChecked;
  const visibleTags = tags.slice(0, MAX_VISIBLE_TAGS);
  const visiblePrs = pullRequests.slice(0, MAX_VISIBLE_PRS);
  const activityIndicator = renderWorkspaceActivity(
    workspaceActivity,
    statusColor
  );

  return (
    <Draggable draggableId={issue.id} index={index}>
      {(provided, snapshot) => (
        <div
          ref={provided.innerRef}
          {...provided.draggableProps}
          role="button"
          tabIndex={0}
          onClick={onClick}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              onClick(e as unknown as MouseEvent);
            }
          }}
          className={cn(
            'group/row flex items-center justify-between gap-double px-double py-half',
            'transition-colors',
            'hover:bg-secondary',
            (isSelected || isChecked) && 'bg-secondary',
            snapshot.isDragging && 'bg-secondary shadow-lg cursor-grabbing',
            className
          )}
        >
          {/* Left side: Drag handle + Checkbox (separate slots), Priority, ID, Status, Title */}
          <div className="flex items-center gap-double flex-1 min-w-0">
            {/* Drag handle and checkbox are siblings in fixed-width slots so
                the surrounding content never shifts. The drag handle stays
                visible at all times — including when other rows are checked
                — so the user can still drag any row in multi-select mode. */}
            <div className="flex items-center gap-half shrink-0">
              {/* Drag handle — always visible. */}
              <div
                {...provided.dragHandleProps}
                className="shrink-0 w-4 flex items-center justify-center cursor-grab"
                onClick={(e) => e.stopPropagation()}
              >
                <DotsSixVerticalIcon
                  className="size-icon-xs text-low"
                  weight="bold"
                />
              </div>
              {/* Checkbox slot — always reserves its space (display: flex);
                  only the icon's paint is toggled via visibility. Visible on
                  row hover, when this row is checked, or whenever multi-select
                  is globally active. */}
              <div
                className={cn(
                  'shrink-0 w-4 flex items-center justify-center',
                  showCheckbox
                    ? 'visible'
                    : 'invisible group-hover/row:visible'
                )}
                onClick={(e) => e.stopPropagation()}
              >
                <Checkbox
                  checked={isChecked}
                  onCheckedChange={(checked) => {
                    onCheckboxChange?.(checked);
                  }}
                />
              </div>
            </div>
            <PriorityIcon priority={issue.priority} />
            <span className="font-ibm-plex-mono text-sm text-normal shrink-0">
              {issue.simple_id}
            </span>
            {activityIndicator}
            <span className="text-base text-high truncate">{issue.title}</span>
          </div>

          {/* Right side: PRs, Tags, Assignee, Age */}
          <div className="flex items-center gap-base shrink-0">
            {(visiblePrs.length > 0 || !!linearUrl) && (
              <div className="flex items-center gap-half">
                {visiblePrs.map((pr) => (
                  <PrBadge
                    key={pr.id}
                    number={pr.number}
                    url={pr.url}
                    status={pr.status}
                  />
                ))}
                {pullRequests.length > MAX_VISIBLE_PRS && (
                  <span className="text-sm text-low">
                    +{pullRequests.length - MAX_VISIBLE_PRS}
                  </span>
                )}
                {linearUrl && <LinearBadge url={linearUrl} />}
              </div>
            )}
            {visibleTags.length > 0 && (
              <div className="flex items-center gap-half">
                {visibleTags.map((tag) => (
                  <KanbanBadge key={tag.id} name={tag.name} color={tag.color} />
                ))}
              </div>
            )}
            {relationships.length > 0 && (
              <div className="flex items-center gap-half">
                {relationships.slice(0, 2).map((rel) => (
                  <RelationshipBadge
                    key={rel.relationshipId}
                    displayType={rel.displayType}
                    relatedIssueDisplayId={rel.relatedIssueDisplayId}
                    compact
                  />
                ))}
                {relationships.length > 2 && (
                  <span className="text-sm text-low">
                    +{relationships.length - 2}
                  </span>
                )}
              </div>
            )}
            <KanbanAssignee assignees={assignees} />
            <span className="text-sm text-low w-5 text-right">
              {formatRelativeTime(issue.created_at)}
            </span>
          </div>
        </div>
      )}
    </Draggable>
  );
}

function renderWorkspaceActivity(
  signal: WorkspaceActivitySignal | null | undefined,
  fallbackStatusColor: string
) {
  switch (signal) {
    case 'pendingApproval':
      return (
        <HandIcon
          className="size-icon-xs text-brand shrink-0"
          weight="fill"
        />
      );
    case 'running':
      return <RunningDots />;
    case 'failed':
      return (
        <TriangleIcon
          className="size-icon-xs text-error shrink-0"
          weight="fill"
        />
      );
    case 'unseenActivity':
      return (
        <CircleIcon
          className="size-icon-xs text-brand shrink-0"
          weight="fill"
        />
      );
    default:
      return <StatusDot color={fallbackStatusColor} />;
  }
}
