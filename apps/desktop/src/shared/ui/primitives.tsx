/** Small owned building blocks styled with the design tokens. */
import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Switch as RadixSwitch, ToggleGroup, Tooltip } from "radix-ui";
import { X } from "lucide-react";

export function cn(...parts: (string | false | null | undefined)[]): string {
  return parts.filter(Boolean).join(" ");
}

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "ghost";
  size?: "sm" | "md";
};

const VARIANTS = {
  primary: "bg-accent text-on-accent hover:bg-accent-hover border-transparent",
  secondary: "bg-surface text-text hover:bg-gray-3 border-border",
  ghost: "bg-transparent text-muted hover:bg-gray-3 hover:text-text border-transparent",
} as const;

export function Button({ variant = "secondary", size = "md", className, ...props }: ButtonProps) {
  return (
    <button
      type="button"
      className={cn(
        "inline-flex items-center justify-center gap-1.5 rounded-control border font-medium whitespace-nowrap",
        "transition-colors duration-150 disabled:pointer-events-none disabled:opacity-50",
        size === "sm" ? "h-7 px-2 text-xs" : "h-8 px-3 text-sm",
        VARIANTS[variant],
        className,
      )}
      {...props}
    />
  );
}

/** A labelled hover/focus hint. Text only: never the sole carrier of information. */
export function Tip({ content, children }: { content: ReactNode; children: ReactNode }) {
  return (
    <Tooltip.Root delayDuration={250}>
      <Tooltip.Trigger asChild>{children}</Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content
          sideOffset={6}
          className="z-50 max-w-72 rounded-control border border-border bg-raised px-2.5 py-1.5 text-xs text-text shadow-overlay"
        >
          {content}
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}

export const TipProvider = Tooltip.Provider;

export function Switch({
  checked,
  onCheckedChange,
  label,
  disabled,
}: {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <RadixSwitch.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      aria-label={label}
      disabled={disabled ?? false}
      className="relative h-5 w-9 shrink-0 rounded-full border border-border-strong bg-gray-4 transition-colors data-[state=checked]:border-accent data-[state=checked]:bg-accent disabled:opacity-50"
    >
      <RadixSwitch.Thumb className="block size-4 translate-x-0.5 rounded-full bg-surface shadow transition-transform duration-150 data-[state=checked]:translate-x-[17px]" />
    </RadixSwitch.Root>
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: readonly { value: T; label: string }[];
  onChange: (value: T) => void;
  label: string;
}) {
  return (
    <ToggleGroup.Root
      type="single"
      value={value}
      aria-label={label}
      onValueChange={(next) => {
        const option = options.find((o) => o.value === next);
        if (option !== undefined) onChange(option.value);
      }}
      className="inline-flex rounded-control border border-border bg-sunken p-0.5"
    >
      {options.map((o) => (
        <ToggleGroup.Item
          key={o.value}
          value={o.value}
          className="h-6 rounded-[4px] px-2 text-xs text-muted transition-colors data-[state=on]:bg-surface data-[state=on]:text-text data-[state=on]:shadow-sm"
        >
          {o.label}
        </ToggleGroup.Item>
      ))}
    </ToggleGroup.Root>
  );
}

export function Chip({
  children,
  onRemove,
  label,
}: {
  children: ReactNode;
  onRemove: () => void;
  label: string;
}) {
  return (
    <span className="inline-flex h-6 items-center gap-1 rounded-full border border-border bg-surface pr-1 pl-2.5 text-xs">
      {children}
      <button
        type="button"
        aria-label={label}
        onClick={onRemove}
        className="grid size-4 place-items-center rounded-full text-subtle hover:bg-gray-4 hover:text-text"
      >
        <X size={12} aria-hidden />
      </button>
    </span>
  );
}

export function Skeleton({ className }: { className?: string }) {
  return <div aria-hidden className={cn("animate-pulse rounded-control bg-gray-3", className)} />;
}

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="rounded border border-border bg-sunken px-1.5 py-0.5 font-mono text-[11px] text-muted">
      {children}
    </kbd>
  );
}

/** A labelled figure inside a description list (`<dl>`). */
export function Stat({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="min-w-0">
      <dt className="text-xs text-subtle">{label}</dt>
      <dd className="truncate text-sm font-medium" data-numeric>
        {value}
      </dd>
    </div>
  );
}
