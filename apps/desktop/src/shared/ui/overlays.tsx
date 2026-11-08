/** Menus, dialogs and side sheets on Radix primitives. */
import type { ReactNode } from "react";
import { Dialog, DropdownMenu } from "radix-ui";
import { Check, X } from "lucide-react";

import { cn } from "./primitives";

const MENU_SURFACE =
  "z-50 min-w-44 rounded-control border border-border bg-raised p-1 text-sm shadow-overlay";
const MENU_ITEM =
  "flex h-8 cursor-default select-none items-center gap-2 rounded-[4px] px-2 outline-none data-[highlighted]:bg-gray-3 data-[disabled]:opacity-50";

export function Menu({
  trigger,
  children,
  label,
}: {
  trigger: ReactNode;
  children: ReactNode;
  /** The trigger's accessible name; it must contain the trigger's visible text (WCAG 2.5.3). */
  label: string;
}) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild aria-label={label}>
        {trigger}
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content align="start" sideOffset={4} className={MENU_SURFACE}>
          {children}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

export function MenuItem({ children, onSelect }: { children: ReactNode; onSelect: () => void }) {
  return (
    <DropdownMenu.Item className={MENU_ITEM} onSelect={onSelect}>
      {children}
    </DropdownMenu.Item>
  );
}

export function MenuRadio<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: readonly { value: T; label: string }[];
  onChange: (value: T) => void;
}) {
  return (
    <DropdownMenu.RadioGroup
      value={value}
      onValueChange={(next) => {
        const option = options.find((o) => o.value === next);
        if (option !== undefined) onChange(option.value);
      }}
    >
      {options.map((o) => (
        <DropdownMenu.RadioItem
          key={o.value}
          value={o.value}
          className={cn(MENU_ITEM, "pl-7 relative")}
        >
          <DropdownMenu.ItemIndicator className="absolute left-2">
            <Check size={14} aria-hidden />
          </DropdownMenu.ItemIndicator>
          {o.label}
        </DropdownMenu.RadioItem>
      ))}
    </DropdownMenu.RadioGroup>
  );
}

export function MenuSeparator() {
  return <DropdownMenu.Separator className="my-1 h-px bg-border" />;
}

export function MenuLabel({ children }: { children: ReactNode }) {
  return (
    <DropdownMenu.Label className="px-2 py-1 text-xs text-subtle">{children}</DropdownMenu.Label>
  );
}

interface OverlayProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  children: ReactNode;
}

/**
 * A centred dialog (command palette, shortcut list). The title is announced; it is shown
 * only with `showTitle`, for dialogs without another visible heading.
 */
export function Modal({
  open,
  onOpenChange,
  title,
  description,
  showTitle = false,
  children,
}: OverlayProps & { showTitle?: boolean }) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/30" />
        <Dialog.Content
          // Without a description, opt out explicitly instead of repeating the title.
          {...(description === undefined ? { "aria-describedby": undefined } : {})}
          className="fixed top-[18%] left-1/2 z-50 w-[min(560px,92vw)] -translate-x-1/2 rounded-sheet border border-border bg-raised shadow-overlay outline-none"
        >
          <Dialog.Title className={showTitle ? "px-5 pt-5 text-md font-semibold" : "sr-only"}>
            {title}
          </Dialog.Title>
          {description === undefined ? null : (
            <Dialog.Description className="sr-only">{description}</Dialog.Description>
          )}
          {children}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

/** A panel sliding in from the right (filters, details). */
export function Sheet({ open, onOpenChange, title, description, children }: OverlayProps) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/20" />
        <Dialog.Content
          {...(description === undefined ? { "aria-describedby": undefined } : {})}
          className="fixed inset-y-0 right-0 z-50 flex w-[min(420px,94vw)] flex-col border-l border-border bg-raised shadow-overlay outline-none"
        >
          <div className="flex items-center gap-2 border-b border-border px-5 py-4">
            <div>
              <Dialog.Title className="text-md font-semibold">{title}</Dialog.Title>
              {description === undefined ? null : (
                <Dialog.Description className="text-xs text-muted">
                  {description}
                </Dialog.Description>
              )}
            </div>
            <Dialog.Close
              aria-label="Close"
              className="ml-auto grid size-7 place-items-center rounded-control text-muted hover:bg-gray-3"
            >
              <X size={16} aria-hidden />
            </Dialog.Close>
          </div>
          <div className="min-h-0 flex-1 overflow-auto px-5 py-4">{children}</div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
