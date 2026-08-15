import * as React from "react";
import { ChevronDown, Check } from "lucide-react";

export type SelectOption = { value: string; label: string; hint?: string };

type Props = {
  value: string;
  onValueChange: (v: string) => void;
  options: SelectOption[];
  placeholder?: string;
  disabled?: boolean;
  className?: string;
};

export function Select({ value, onValueChange, options, placeholder = "Select…", disabled, className = "" }: Props) {
  const [open, setOpen] = React.useState(false);
  const ref = React.useRef<HTMLDivElement>(null);
  const selected = options.find((o) => o.value === value);

  React.useEffect(() => {
    const onDoc = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, []);

  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    if (open) document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open]);

  return (
    <div ref={ref} className={`relative ${className}`}>
      <button
        type="button"
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
        className="flex h-9 w-full items-center justify-between rounded-lg border border-zinc-200 bg-white px-3 text-sm text-zinc-900 hover:bg-zinc-50 disabled:opacity-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100 dark:hover:bg-zinc-800"
      >
        <span className="truncate">{selected?.label ?? placeholder}</span>
        <ChevronDown className={`h-4 w-4 shrink-0 opacity-50 transition-transform ${open ? "rotate-180" : ""}`} />
      </button>
      {open && (
        <div className="absolute z-50 mt-1 max-h-60 w-full overflow-auto rounded-lg border border-zinc-200 bg-white p-1 shadow-lg dark:border-zinc-700 dark:bg-zinc-900">
          {options.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => {
                onValueChange(opt.value);
                setOpen(false);
              }}
              className={`flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800 ${opt.value === value ? "bg-zinc-100 dark:bg-zinc-800" : ""}`}
            >
              <span>
                {opt.label}
                {opt.hint && <span className="ml-1.5 text-xs text-zinc-500">{opt.hint}</span>}
              </span>
              {opt.value === value && <Check className="h-4 w-4" />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
