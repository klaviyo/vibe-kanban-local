import { useState, useEffect, useMemo, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@vibe/ui/components/KeyboardDialog';
import { Button } from '@vibe/ui/components/Button';
import { Input } from '@vibe/ui/components/Input';
import { Label } from '@vibe/ui/components/Label';
import { create, useModal } from '@ebay/nice-modal-react';
import { defineModal } from '@/shared/lib/modals';
import { useProjectContext } from '@/shared/hooks/useProjectContext';
import { ProjectProvider } from '@/shared/providers/remote/ProjectProvider';
import { parseLinearTicketId } from '@vibe/ui/components/LinearBadge';

export interface LinkLinearToIssueDialogProps {
  /** Project the issue belongs to. Required because nice-modal-react
   *  portals the dialog outside the kanban's `ProjectProvider`, so the
   *  dialog has to mount its own provider to read/write issue mutations. */
  projectId: string;
  issueId: string;
  /** Current value (if any) so the dialog opens pre-populated for editing. */
  currentUrl?: string | null;
}

function LinkLinearToIssueContent({
  issueId,
  currentUrl,
}: Omit<LinkLinearToIssueDialogProps, 'projectId'>) {
  const modal = useModal();
  const { t } = useTranslation('tasks');

  const [linearUrl, setLinearUrl] = useState(currentUrl ?? '');
  // nice-modal-react keeps the dialog mounted across `.show()` calls, so
  // the initial-value capture has to happen on each visibility-open
  // transition, not once at mount. Track the latest prop in a ref the
  // open-effect reads, so a `currentUrl` change while the dialog is
  // already open does not stomp the user's in-flight edit.
  const latestCurrentUrl = useRef(currentUrl ?? '');
  useEffect(() => {
    latestCurrentUrl.current = currentUrl ?? '';
  }, [currentUrl]);

  const { updateIssue } = useProjectContext();
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Seed (or re-seed) the input from `currentUrl` whenever the dialog
  // opens. While it's closed we leave state alone — nothing reads it.
  useEffect(() => {
    if (modal.visible) {
      setLinearUrl(latestCurrentUrl.current);
      setError(null);
    }
  }, [modal.visible]);

  const trimmed = linearUrl.trim();
  const isClearing = trimmed === '';
  const ticketId = useMemo(
    () => (isClearing ? null : parseLinearTicketId(trimmed)),
    [trimmed, isClearing]
  );
  const validationError = useMemo<string | null>(() => {
    if (isClearing) return null;
    if (!/^https?:\/\/linear\.app\//i.test(trimmed)) {
      return t(
        'linkLinearToIssue.errors.notLinearUrl',
        'URL must start with https://linear.app/'
      );
    }
    if (!ticketId) {
      return t(
        'linkLinearToIssue.errors.unrecognizedShape',
        "Couldn't find a ticket id (e.g. 'TEAM-123') in that URL"
      );
    }
    return null;
  }, [trimmed, isClearing, ticketId, t]);

  const handleOpenChange = (open: boolean) => {
    if (!open) modal.hide();
  };

  // Compare against the live `currentUrl` prop (always the persisted
  // truth) — the ref is only used to seed input on open.
  const persistedTrimmed = (currentUrl ?? '').trim();
  const canSubmit =
    !isSaving && !validationError && trimmed !== persistedTrimmed;

  const handleSubmit = useCallback(async () => {
    if (!canSubmit) return;
    setIsSaving(true);
    setError(null);
    try {
      // RFC 7396 merge-PATCH on the server: passing `null` deletes the
      // key, leaving siblings in `extension_metadata` untouched. Passing a
      // string overwrites the key. We never need to read-then-write.
      const { persisted } = updateIssue(issueId, {
        extension_metadata: { linear_url: isClearing ? null : trimmed },
      });
      await persisted;
      modal.hide();
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : t('linkLinearToIssue.errors.failed', 'Failed to save Linear link')
      );
    } finally {
      setIsSaving(false);
    }
  }, [canSubmit, updateIssue, issueId, isClearing, trimmed, modal, t]);

  return (
    <Dialog open={modal.visible} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-[500px]">
        <DialogHeader>
          <DialogTitle>
            {t('linkLinearToIssue.title', 'Link Linear ticket')}
          </DialogTitle>
          <DialogDescription>
            {t(
              'linkLinearToIssue.description',
              "Paste the full Linear ticket URL. Leaving it blank removes any existing link."
            )}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-4">
          <div className="space-y-2">
            <Label>
              {t('linkLinearToIssue.urlLabel', 'Linear ticket URL')}
            </Label>
            <Input
              placeholder="https://linear.app/your-org/issue/TEAM-123/..."
              value={linearUrl}
              onChange={(e) => setLinearUrl(e.target.value)}
              autoFocus
            />
          </div>

          {validationError && (
            <div className="text-sm text-destructive">{validationError}</div>
          )}

          {!validationError && ticketId && (
            <div className="text-sm text-muted-foreground">
              {t('linkLinearToIssue.detected', 'Detected ticket:')}{' '}
              <span className="font-medium text-foreground">{ticketId}</span>
            </div>
          )}

          {!validationError && isClearing && persistedTrimmed !== '' && (
            <div className="text-sm text-muted-foreground">
              {t(
                'linkLinearToIssue.willClear',
                'Submitting will remove the existing Linear link.'
              )}
            </div>
          )}

          {error && <div className="text-sm text-destructive">{error}</div>}
        </div>

        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => modal.hide()}
            disabled={isSaving}
          >
            {t('common:buttons.cancel')}
          </Button>
          <Button onClick={handleSubmit} disabled={!canSubmit}>
            {isSaving
              ? t('linkLinearToIssue.saving', 'Saving…')
              : isClearing
                ? t('linkLinearToIssue.removeLink', 'Remove link')
                : t('linkLinearToIssue.saveLink', 'Save link')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function LinkLinearToIssueWithContext({
  projectId,
  issueId,
  currentUrl,
}: LinkLinearToIssueDialogProps) {
  if (!projectId) return null;
  return (
    <ProjectProvider projectId={projectId}>
      <LinkLinearToIssueContent issueId={issueId} currentUrl={currentUrl} />
    </ProjectProvider>
  );
}

const LinkLinearToIssueDialogImpl = create<LinkLinearToIssueDialogProps>(
  ({ projectId, issueId, currentUrl }) => {
    return (
      <LinkLinearToIssueWithContext
        projectId={projectId}
        issueId={issueId}
        currentUrl={currentUrl}
      />
    );
  }
);

export const LinkLinearToIssueDialog = defineModal<
  LinkLinearToIssueDialogProps,
  void
>(LinkLinearToIssueDialogImpl);
