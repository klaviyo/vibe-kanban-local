import { useCallback, useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import { APP_BAR_TOOL_DRAG_TYPE } from '@vibe/ui/components/AppBar';
import { ToolMenu } from './ToolMenu';
import { useTools } from './useTools';

const ISSUE_SELECTOR = '[data-issue-id]';
// A background tint, not a border: borders on adjacent rows read as an
// insertion marker rather than a drop target.
const HOVER_BACKGROUND = 'hsl(var(--brand) / 0.2)';

interface PendingDrop {
  toolId: string;
  issueId: string;
  x: number;
  y: number;
}

function issueElementAt(target: EventTarget | null): HTMLElement | null {
  return target instanceof Element
    ? target.closest<HTMLElement>(ISSUE_SELECTOR)
    : null;
}

function isToolDrag(e: DragEvent): boolean {
  return e.dataTransfer?.types.includes(APP_BAR_TOOL_DRAG_TYPE) ?? false;
}

/**
 * Lets a sidebar tool button be dropped onto any ticket (kanban card or list
 * row, marked with `data-issue-id`) and shows that tool's ticket options.
 */
export function ToolTicketDrop() {
  const { tools } = useTools();
  const [pending, setPending] = useState<PendingDrop | null>(null);

  useEffect(() => {
    let hovered: HTMLElement | null = null;
    const clearHover = () => {
      if (hovered) hovered.style.backgroundColor = '';
      hovered = null;
    };

    const onDragOver = (e: DragEvent) => {
      if (!isToolDrag(e)) return;
      const el = issueElementAt(e.target);
      if (!el) {
        clearHover();
        return;
      }
      e.preventDefault();
      if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy';
      if (hovered !== el) {
        clearHover();
        hovered = el;
        el.style.backgroundColor = HOVER_BACKGROUND;
      }
    };
    const onDrop = (e: DragEvent) => {
      if (!isToolDrag(e)) return;
      const el = issueElementAt(e.target);
      clearHover();
      const toolId = e.dataTransfer?.getData(APP_BAR_TOOL_DRAG_TYPE);
      const issueId = el?.dataset.issueId;
      if (!toolId || !issueId) return;
      e.preventDefault();
      setPending({ toolId, issueId, x: e.clientX, y: e.clientY });
    };

    document.addEventListener('dragover', onDragOver, true);
    document.addEventListener('drop', onDrop, true);
    document.addEventListener('dragend', clearHover, true);
    return () => {
      clearHover();
      document.removeEventListener('dragover', onDragOver, true);
      document.removeEventListener('drop', onDrop, true);
      document.removeEventListener('dragend', clearHover, true);
    };
  }, []);

  const close = useCallback(() => setPending(null), []);

  useEffect(() => {
    if (!pending) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') close();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [pending, close]);

  const tool = pending ? tools.find((t) => t.id === pending.toolId) : null;
  if (!pending || !tool) return null;

  return createPortal(
    <>
      <div className="fixed inset-0 z-[10000]" onMouseDown={close} />
      <div
        className="fixed z-[10001] rounded-sm border border-border bg-panel p-1 shadow-md"
        style={{
          left: Math.min(pending.x, window.innerWidth - 340),
          top: Math.min(pending.y, window.innerHeight - 200),
        }}
      >
        <ToolMenu
          tool={tool}
          target="ticket"
          issueId={pending.issueId}
          onDone={close}
        />
      </div>
    </>,
    document.body
  );
}
