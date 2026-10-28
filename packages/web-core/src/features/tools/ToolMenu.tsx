import { useQuery } from '@tanstack/react-query';
import { SpinnerIcon } from '@phosphor-icons/react';
import { cn } from '@/shared/lib/utils';
import { toolsApi, type MenuItem, type MenuTone, type Tool } from './toolsApi';
import { useTools } from './useTools';

const TONE_CLASS: Record<MenuTone, string> = {
  ok: 'bg-success',
  warn: 'bg-warning',
  error: 'bg-error',
  info: 'bg-low',
};

interface ToolMenuProps {
  tool: Tool;
  target: 'click' | 'ticket';
  issueId?: string;
  /** Called when an option is chosen, so the host can close its popup. */
  onDone: () => void;
}

/**
 * A tool's menu: static options, or items produced by its `options_command`
 * (clickable options plus status text, headers and separators). A dynamic menu
 * is fetched fresh each time this mounts, i.e. each time the menu opens.
 */
export function ToolMenu({ tool, target, issueId, onDone }: ToolMenuProps) {
  const { runTool } = useTools();
  const spec = target === 'click' ? tool.click : tool.ticket;

  const menu = useQuery({
    queryKey: ['tool-menu', tool.id, target, issueId],
    queryFn: () =>
      toolsApi.menu({ tool_id: tool.id, target, issue_id: issueId }),
    enabled: spec.dynamic,
    gcTime: 0,
    staleTime: 0,
    retry: false,
  });

  const staticItems: MenuItem[] = spec.options.map((o) => ({
    type: 'option',
    id: o.id,
    title: o.title,
    disabled: false,
  }));
  const items = !spec.dynamic
    ? staticItems
    : menu.isSuccess
      ? menu.data
      : menu.isError
        ? staticItems
        : [];

  const choose = (optionId: string) => {
    onDone();
    void runTool(tool, {
      target,
      option_id: optionId,
      issue_id: issueId,
    });
  };

  return (
    <div role="menu" className="min-w-48 max-w-80">
      <p className="px-base py-half text-xs font-medium text-low">
        {tool.title}
      </p>
      {spec.dynamic && menu.isPending && (
        <p className="flex items-center gap-half px-base py-half text-sm text-low">
          <SpinnerIcon className="size-icon-xs animate-spin" weight="bold" />
          Loading…
        </p>
      )}
      {menu.isError && (
        <p className="px-base py-half text-xs text-error whitespace-pre-wrap break-words">
          {menu.error instanceof Error ? menu.error.message : 'Menu failed'}
        </p>
      )}
      {spec.dynamic && menu.isSuccess && items.length === 0 && (
        <p className="px-base py-half text-sm text-low">Nothing to do</p>
      )}
      {items.map((item, index) => {
        switch (item.type) {
          case 'header':
            return (
              <p
                key={index}
                className="px-base pt-half text-xs font-medium uppercase tracking-wide text-low"
              >
                {item.text}
              </p>
            );
          case 'text':
            return (
              <p
                key={index}
                className="flex items-start gap-half px-base py-half text-sm text-normal whitespace-pre-wrap break-words"
              >
                {item.tone && (
                  <span
                    className={cn(
                      'mt-[0.4em] size-2 shrink-0 rounded-full',
                      TONE_CLASS[item.tone]
                    )}
                  />
                )}
                <span>{item.text}</span>
              </p>
            );
          case 'separator':
            return <div key={index} className="my-1 border-t border-border" />;
          case 'option':
            return (
              <button
                key={index}
                type="button"
                role="menuitem"
                disabled={item.disabled}
                onClick={() => choose(item.id)}
                className={cn(
                  'block w-full rounded-sm px-base py-half text-left text-sm',
                  item.disabled
                    ? 'cursor-not-allowed text-low opacity-50'
                    : 'text-normal hover:bg-secondary'
                )}
              >
                {item.title}
              </button>
            );
        }
      })}
    </div>
  );
}
