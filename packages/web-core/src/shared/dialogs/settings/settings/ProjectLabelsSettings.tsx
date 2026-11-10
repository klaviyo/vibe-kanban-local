import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PencilSimpleLineIcon, XIcon } from '@phosphor-icons/react';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@vibe/ui/components/Popover';
import { InlineColorPicker } from '@vibe/ui/components/ColorPicker';
import { useShape } from '@/shared/integrations/electric/hooks';
import {
  PROJECT_ISSUE_TAGS_SHAPE,
  PROJECT_TAGS_SHAPE,
  TAG_MUTATION,
  type Tag,
} from 'shared/remote-types';
import { PRESET_COLORS } from '@/shared/lib/colors';

interface LabelRowProps {
  tag: Tag;
  isNameTaken: (name: string) => boolean;
  onRename: (name: string) => void;
  onRecolor: (color: string) => void;
  onDelete: () => void;
}

function LabelRow({
  tag,
  isNameTaken,
  onRename,
  onRecolor,
  onDelete,
}: LabelRowProps) {
  const { t } = useTranslation('common');
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(tag.name);
  const [colorOpen, setColorOpen] = useState(false);

  const trimmed = name.trim();
  const duplicate = trimmed !== tag.name && isNameTaken(trimmed);

  const finish = (commit: boolean) => {
    if (commit && trimmed && trimmed !== tag.name && !duplicate) {
      onRename(trimmed);
    } else {
      setName(tag.name);
    }
    setEditing(false);
  };

  return (
    <div className="flex items-center justify-between px-base py-half rounded-sm bg-secondary">
      <div className="flex items-center gap-base">
        <Popover open={colorOpen} onOpenChange={setColorOpen}>
          <PopoverTrigger asChild>
            <button
              type="button"
              className="flex items-center justify-center size-icon-sm"
              title={t('kanban.changeColor', 'Change color')}
            >
              <div
                className="size-dot rounded-full shrink-0"
                style={{ backgroundColor: `hsl(${tag.color})` }}
              />
            </button>
          </PopoverTrigger>
          <PopoverContent align="start" className="w-auto p-base">
            <InlineColorPicker
              value={tag.color}
              onChange={(color) => {
                onRecolor(color);
                setColorOpen(false);
              }}
              colors={PRESET_COLORS}
            />
          </PopoverContent>
        </Popover>

        {editing ? (
          <div className="flex flex-col">
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  finish(true);
                } else if (e.key === 'Escape') {
                  finish(false);
                }
              }}
              onBlur={() => finish(true)}
              autoFocus
              className="bg-transparent text-sm text-high outline-none border-b border-brand w-40"
            />
            {duplicate && (
              <span className="text-xs text-error">
                {t('kanban.labelExists', 'A label with this name exists')}
              </span>
            )}
          </div>
        ) : (
          <span
            className="text-sm text-high cursor-pointer"
            onClick={() => setEditing(true)}
          >
            {tag.name}
          </span>
        )}
      </div>

      <div className="flex items-center gap-base">
        <button
          type="button"
          onClick={() => setEditing(true)}
          className="flex items-center justify-center size-icon-sm text-low hover:text-normal"
          title={t('kanban.editName', 'Edit name')}
        >
          <PencilSimpleLineIcon className="size-icon-xs" weight="bold" />
        </button>
        <button
          type="button"
          onClick={onDelete}
          className="flex items-center justify-center size-icon-sm text-low hover:text-normal"
          title={t('kanban.deleteLabel', 'Delete label')}
        >
          <XIcon className="size-icon-xs" weight="bold" />
        </button>
      </div>
    </div>
  );
}

/** Rename, recolor and delete a project's labels. Changes apply immediately. */
export function ProjectLabelsSettings({ projectId }: { projectId: string }) {
  const { t } = useTranslation('common');
  const params = useMemo(() => ({ project_id: projectId }), [projectId]);
  const {
    data: tags,
    update,
    remove,
  } = useShape(PROJECT_TAGS_SHAPE, params, { mutation: TAG_MUTATION });
  const { data: issueTags } = useShape(PROJECT_ISSUE_TAGS_SHAPE, params);
  const [error, setError] = useState<string | null>(null);

  const sorted = useMemo(
    () => [...tags].sort((a, b) => a.name.localeCompare(b.name)),
    [tags]
  );

  const report = (result: { persisted: Promise<unknown> }) =>
    result.persisted.then(
      () => setError(null),
      (e) => setError(e instanceof Error ? e.message : String(e))
    );

  const handleDelete = (tag: Tag) => {
    const count = new Set(
      issueTags.filter((it) => it.tag_id === tag.id).map((it) => it.issue_id)
    ).size;
    const confirmed = window.confirm(
      t('kanban.deleteLabelConfirm', {
        defaultValue:
          count === 1
            ? 'Delete label "{{name}}"? It will be removed from 1 ticket.'
            : 'Delete label "{{name}}"? It will be removed from {{count}} tickets.',
        name: tag.name,
        count,
      })
    );
    if (confirmed) void report(remove(tag.id));
  };

  return (
    <div className="bg-secondary/50 border border-border rounded-sm p-4 space-y-base">
      <div>
        <p className="text-sm font-medium text-normal">
          {t('kanban.labels', 'Labels')}
        </p>
        <p className="text-sm text-low mt-1">
          {t(
            'kanban.labelsDescription',
            'Rename, recolor or delete labels for this project.'
          )}
        </p>
      </div>
      {error && <p className="text-xs text-error">{error}</p>}
      <div className="flex flex-col gap-[2px]">
        {sorted.length === 0 && (
          <p className="text-sm text-low">
            {t('kanban.noLabels', 'No labels yet')}
          </p>
        )}
        {sorted.map((tag) => (
          <LabelRow
            key={tag.id}
            tag={tag}
            isNameTaken={(name) =>
              tags.some(
                (o) =>
                  o.id !== tag.id && o.name.toLowerCase() === name.toLowerCase()
              )
            }
            onRename={(name) => void report(update(tag.id, { name }))}
            onRecolor={(color) => void report(update(tag.id, { color }))}
            onDelete={() => handleDelete(tag)}
          />
        ))}
      </div>
    </div>
  );
}
