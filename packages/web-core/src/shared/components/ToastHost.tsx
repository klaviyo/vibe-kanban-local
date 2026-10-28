import { XIcon } from '@phosphor-icons/react';
import { cn } from '@/shared/lib/utils';
import { useToastStore } from '@/shared/stores/useToastStore';

export function ToastHost() {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);

  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-6 right-6 z-[10001] flex w-80 flex-col gap-base">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          role="status"
          className={cn(
            'rounded-sm border bg-panel p-base shadow-md',
            toast.tone === 'error' ? 'border-error' : 'border-border'
          )}
        >
          <div className="flex items-start justify-between gap-base">
            <p
              className={cn(
                'text-sm font-medium',
                toast.tone === 'error' ? 'text-error' : 'text-high'
              )}
            >
              {toast.title}
            </p>
            <button
              type="button"
              aria-label="Dismiss"
              onClick={() => dismiss(toast.id)}
              className="text-low hover:text-normal"
            >
              <XIcon className="size-icon-xs" weight="bold" />
            </button>
          </div>
          {toast.detail && (
            <pre className="mt-half max-h-32 overflow-auto whitespace-pre-wrap break-words text-xs text-low">
              {toast.detail}
            </pre>
          )}
        </div>
      ))}
    </div>
  );
}
