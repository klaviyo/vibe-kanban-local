import { useEffect, useRef, useState, type ReactNode } from 'react';
import { CheckIcon, PlusIcon } from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import {
  Command,
  CommandInput,
  CommandList,
  CommandEmpty,
  CommandGroup,
  CommandItem,
} from './Command';
import { PrimaryButton } from './PrimaryButton';
import { InlineColorPicker, PRESET_COLORS } from './ColorPicker';

export interface MultiSelectOption<T extends string = string> {
  value: T;
  label: string;
  searchValue?: string;
  renderOption?: () => ReactNode;
}

export interface MultiSelectCreate {
  /** True when `name` is already an option, so it can't be created again. */
  exists: (name: string) => boolean;
  /** Called with the typed name and the color picked for it. */
  onCreate: (name: string, color: string) => void;
}

interface MultiSelectCommandBarProps<T extends string = string> {
  title: string;
  options: MultiSelectOption<T>[];
  selectedValues: T[];
  onToggle: (value: T) => void;
  onClose: () => void;
  search: string;
  onSearchChange: (search: string) => void;
  /** Offers a "Create" row (then a color step) for text matching no option. */
  create?: MultiSelectCreate;
}

export function MultiSelectCommandBar<T extends string = string>({
  title,
  options,
  selectedValues,
  onToggle,
  onClose,
  search,
  onSearchChange,
  create,
}: MultiSelectCommandBarProps<T>) {
  const { t } = useTranslation('common');
  const [isCreating, setIsCreating] = useState(false);
  const [colorIndex, setColorIndex] = useState(0);
  const colorPickerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (isCreating) colorPickerRef.current?.focus();
  }, [isCreating]);

  const newName = search.trim();
  const showCreate = !!create && newName.length > 0 && !create.exists(newName);

  const confirmCreate = () => {
    if (!create || !newName) return;
    create.onCreate(newName, PRESET_COLORS[colorIndex]);
    setIsCreating(false);
    setColorIndex(0);
  };

  if (isCreating) {
    return (
      <div
        ref={colorPickerRef}
        tabIndex={-1}
        className="rounded-sm border border-border p-base space-y-base outline-none"
        onKeyDown={(e) => {
          if (e.key === 'ArrowLeft') {
            e.preventDefault();
            setColorIndex(
              (i) => (i - 1 + PRESET_COLORS.length) % PRESET_COLORS.length
            );
          } else if (e.key === 'ArrowRight') {
            e.preventDefault();
            setColorIndex((i) => (i + 1) % PRESET_COLORS.length);
          } else if (e.key === 'Enter') {
            e.preventDefault();
            confirmCreate();
          }
        }}
      >
        <div className="text-sm text-normal">
          {t('kanban.selectColorFor')}{' '}
          <span className="font-medium">{newName}</span>
        </div>
        <InlineColorPicker
          value={PRESET_COLORS[colorIndex]}
          onChange={(color) =>
            setColorIndex(Math.max(0, PRESET_COLORS.indexOf(color as never)))
          }
          colors={PRESET_COLORS}
        />
        <div className="flex items-center justify-end gap-half pt-half">
          <button
            type="button"
            onClick={() => setIsCreating(false)}
            className="px-base py-half text-sm text-low hover:text-normal hover:bg-panel rounded-sm transition-colors"
          >
            {t('buttons.cancel')}
          </button>
          <button
            type="button"
            onClick={confirmCreate}
            className="px-base py-half text-sm text-high bg-brand hover:bg-brand/90 rounded-sm transition-colors"
          >
            {t('buttons.create')}
          </button>
        </div>
      </div>
    );
  }

  return (
    <Command
      className="rounded-sm border border-border [&_[cmdk-group-heading]]:px-base [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:text-low [&_[cmdk-group]:not([hidden])_~[cmdk-group]]:pt-0 [&_[cmdk-group]]:px-half [&_[cmdk-input-wrapper]_svg]:h-4 [&_[cmdk-input-wrapper]_svg]:w-4 [&_[cmdk-input]]:h-12 [&_[cmdk-item]]:px-base [&_[cmdk-item]]:py-half"
      loop
      filter={(value, search) => {
        if (value.toLowerCase().includes(search.toLowerCase())) return 1;
        return 0;
      }}
      onKeyDown={(e) => {
        if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
          e.preventDefault();
          onClose();
        }
      }}
    >
      <div className="flex items-center border-b border-border">
        <CommandInput
          placeholder={title}
          value={search}
          onValueChange={onSearchChange}
        />
      </div>
      <CommandList className="min-h-[200px]">
        {!showCreate && (
          <CommandEmpty>
            {t('commandBar.noResults', 'No results found')}
          </CommandEmpty>
        )}
        <CommandGroup>
          {options.map((option) => {
            const isSelected = selectedValues.includes(option.value);
            return (
              <CommandItem
                key={option.value}
                value={option.searchValue ?? `${option.value} ${option.label}`}
                onSelect={() => onToggle(option.value)}
              >
                <div className="flex items-center gap-base flex-1">
                  {option.renderOption?.() ?? <span>{option.label}</span>}
                </div>
                {isSelected && (
                  <CheckIcon className="h-4 w-4 text-brand" weight="bold" />
                )}
              </CommandItem>
            );
          })}
        </CommandGroup>
        {showCreate && (
          <CommandGroup>
            <CommandItem
              value={`create ${newName}`}
              onSelect={() => setIsCreating(true)}
            >
              <div className="flex items-center gap-base flex-1 text-brand">
                <PlusIcon className="size-icon-sm" weight="bold" />
                <span>
                  {t('kanban.createTag')} &quot;{newName}&quot;
                </span>
              </div>
            </CommandItem>
          </CommandGroup>
        )}
      </CommandList>
      <div className="border-t border-border p-base">
        <PrimaryButton onClick={onClose} className="w-full justify-center">
          {t('buttons.close')}
        </PrimaryButton>
      </div>
    </Command>
  );
}
