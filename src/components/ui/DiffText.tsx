import * as React from "react";
import { diffWords } from "diff";

// Inline word-level diff of original vs result: removed words struck
// through, added words highlighted. Purely presentational — the plain
// result string remains the source of truth for accept/copy.
export function DiffText({ original, result }: { original: string; result: string }) {
  const chunks = React.useMemo(() => diffWords(original, result), [original, result]);

  return (
    <p className="whitespace-pre-wrap text-[13px] leading-relaxed text-zinc-800 dark:text-zinc-100">
      {chunks.map((c, i) => {
        if (c.removed) {
          return (
            <span
              key={i}
              className="text-red-500/70 line-through decoration-red-400/60 dark:text-red-400/70 dark:decoration-red-500/50"
            >
              {c.value}
            </span>
          );
        }
        if (c.added) {
          return (
            <span
              key={i}
              className="rounded-[2px] bg-emerald-100 text-emerald-900 dark:bg-emerald-900/40 dark:text-emerald-200"
            >
              {c.value}
            </span>
          );
        }
        return <span key={i}>{c.value}</span>;
      })}
    </p>
  );
}
