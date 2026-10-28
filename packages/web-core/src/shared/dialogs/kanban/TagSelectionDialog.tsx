import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { create, useModal } from '@ebay/nice-modal-react';
import { useTranslation } from 'react-i18next';
import { defineModal } from '@/shared/lib/modals';
import { CommandDialog } from '@vibe/ui/components/Command';
import {
  MultiSelectCommandBar,
  type MultiSelectOption,
} from '@vibe/ui/components/MultiSelectCommandBar';
import { ProjectProvider } from '@/shared/providers/remote/ProjectProvider';
import { useProjectContext } from '@/shared/hooks/useProjectContext';

export interface TagSelectionDialogProps {
  projectId: string;
  issueIds: string[];
}

function TagSelectionContent({ issueIds }: { issueIds: string[] }) {
  const { t } = useTranslation('common');
  const modal = useModal();
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const [search, setSearch] = useState('');
  const { tags, issueTags, insertIssueTag, removeIssueTag } =
    useProjectContext();

  // A tag counts as selected when any selected issue has it; toggling a
  // selected tag removes it from every selected issue, otherwise adds it to
  // every selected issue that lacks it.
  const selectedIds = useMemo(
    () => [
      ...new Set(
        issueTags
          .filter((link) => issueIds.includes(link.issue_id))
          .map((link) => link.tag_id)
      ),
    ],
    [issueTags, issueIds]
  );

  useEffect(() => {
    if (modal.visible) {
      previousFocusRef.current = document.activeElement as HTMLElement;
      setSearch('');
    }
  }, [modal.visible]);

  const options: MultiSelectOption<string>[] = useMemo(
    () =>
      tags.map((tag) => ({
        value: tag.id,
        label: tag.name,
        searchValue: tag.name,
        renderOption: () => (
          <div className="flex items-center gap-base">
            <span
              className="size-2 rounded-full shrink-0"
              style={{ backgroundColor: tag.color }}
            />
            <span>{tag.name}</span>
          </div>
        ),
      })),
    [tags]
  );

  const handleToggle = useCallback(
    (tagId: string) => {
      const isSelected = selectedIds.includes(tagId);
      for (const issueId of issueIds) {
        const existing = issueTags.find(
          (link) => link.issue_id === issueId && link.tag_id === tagId
        );
        if (isSelected && existing) {
          removeIssueTag(existing.id);
        } else if (!isSelected && !existing) {
          insertIssueTag({ issue_id: issueId, tag_id: tagId });
        }
      }
      setSearch('');
    },
    [selectedIds, issueIds, issueTags, insertIssueTag, removeIssueTag]
  );

  const handleCloseAutoFocus = useCallback((event: Event) => {
    event.preventDefault();
    previousFocusRef.current?.focus();
  }, []);

  return (
    <CommandDialog
      open={modal.visible}
      onOpenChange={(open) => !open && modal.hide()}
      onCloseAutoFocus={handleCloseAutoFocus}
    >
      <MultiSelectCommandBar
        title={t('kanban.selectTags', 'Select tags...')}
        options={options}
        selectedValues={selectedIds}
        onToggle={handleToggle}
        onClose={() => modal.hide()}
        search={search}
        onSearchChange={setSearch}
      />
    </CommandDialog>
  );
}

const TagSelectionDialogImpl = create<TagSelectionDialogProps>(
  ({ projectId, issueIds }) => (
    <ProjectProvider projectId={projectId}>
      <TagSelectionContent issueIds={issueIds} />
    </ProjectProvider>
  )
);

export const TagSelectionDialog = defineModal<TagSelectionDialogProps, void>(
  TagSelectionDialogImpl
);
