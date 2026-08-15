import * as React from "react";
import { Settings2, Cpu, History, Feather } from "lucide-react";

export type Section = "general" | "model" | "history";

const ITEMS: { id: Section; label: string; icon: typeof Settings2 }[] = [
  { id: "general", label: "General", icon: Settings2 },
  { id: "model", label: "Model", icon: Cpu },
  { id: "history", label: "History", icon: History },
];

export function Sidebar({
  current,
  onSelect,
}: {
  current: Section;
  onSelect: (s: Section) => void;
}) {
  return (
    <aside className="flex w-44 shrink-0 flex-col border-r border-zinc-200 bg-white/60 dark:border-zinc-800 dark:bg-zinc-900/40">
      <div className="flex items-center gap-2 px-4 pb-4 pt-5">
        <Feather className="h-4 w-4 text-zinc-400" />
        <span className="text-sm font-semibold tracking-tight text-zinc-900 dark:text-white">
          Quill
        </span>
      </div>
      <nav className="flex flex-col gap-0.5 px-2">
        {ITEMS.map((item) => {
          const active = item.id === current;
          return (
            <button
              key={item.id}
              onClick={() => onSelect(item.id)}
              className={`flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm transition-colors ${
                active
                  ? "bg-zinc-900 font-medium text-white dark:bg-zinc-100 dark:text-zinc-900"
                  : "text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800"
              }`}
            >
              <item.icon className="h-4 w-4" />
              {item.label}
            </button>
          );
        })}
      </nav>
      <div className="mt-auto px-4 pb-4 text-[10px] leading-relaxed text-zinc-400">
        local-only · Ollama
        <br />
        no cloud keys
      </div>
    </aside>
  );
}
