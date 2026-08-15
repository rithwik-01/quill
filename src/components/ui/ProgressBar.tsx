import * as React from "react";

type Props = {
  value: number; // 0-100
  className?: string;
  showLabel?: boolean;
  size?: "sm" | "md";
};

export function ProgressBar({ value, className = "", showLabel = false, size = "md" }: Props) {
  const pct = Math.max(0, Math.min(100, Math.round(value)));
  return (
    <div className={`flex items-center gap-2 ${className}`}>
      <div className={`flex-1 overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-800 ${size === "sm" ? "h-1.5" : "h-2"}`}>
        <div
          className="h-full rounded-full bg-zinc-900 transition-all duration-300 dark:bg-white"
          style={{ width: `${pct}%` }}
          role="progressbar"
          aria-valuenow={pct}
          aria-valuemin={0}
          aria-valuemax={100}
        />
      </div>
      {showLabel && <span className="min-w-10 text-right text-xs tabular-nums text-zinc-500">{pct}%</span>}
    </div>
  );
}
