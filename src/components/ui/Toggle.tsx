import * as React from "react";

type Props = {
  checked: boolean;
  onCheckedChange: (v: boolean) => void;
  disabled?: boolean;
  id?: string;
  label?: string;
};

export function Toggle({ checked, onCheckedChange, disabled, id, label }: Props) {
  return (
    <label htmlFor={id} className={`inline-flex items-center gap-2 ${disabled ? "opacity-50" : "cursor-pointer"}`}>
      <button
        id={id}
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        type="button"
        onClick={() => onCheckedChange(!checked)}
        className={`relative inline-flex h-6 w-11 shrink-0 rounded-full border-2 border-transparent transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-zinc-400 ${checked ? "bg-zinc-900 dark:bg-white" : "bg-zinc-200 dark:bg-zinc-700"}`}
      >
        <span className={`pointer-events-none block h-5 w-5 rounded-full bg-white shadow transition-transform dark:bg-zinc-900 ${checked ? "translate-x-5 dark:bg-zinc-100" : "translate-x-0"}`} />
      </button>
      {label && <span className="text-sm text-zinc-700 dark:text-zinc-300">{label}</span>}
    </label>
  );
}
