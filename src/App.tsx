import * as React from "react";
import { Toaster, toast } from "sonner";
import { Onboarding } from "./components/onboarding/Onboarding";
import { Sidebar, type Section } from "./components/settings/Sidebar";
import { GeneralSettings } from "./components/settings/GeneralSettings";
import { ModelSettings } from "./components/settings/ModelSettings";
import { HistorySettings } from "./components/settings/HistorySettings";
import { useSettingsStore } from "./stores/settingsStore";
import { useOllamaStore } from "./stores/ollamaStore";
import { Loader2 } from "lucide-react";

/**
 * App routing between onboarding and the tabbed main window (PLAN §6, v1.1).
 * - On first launch (onboardingComplete === false) → Onboarding
 * - Otherwise → sidebar shell: General | Model | History
 * Hydrates settings via tauri-specta bindings (src/bindings.ts) on mount.
 */
export default function App() {
  const { settings, hydrated, hydrate } = useSettingsStore();
  const refreshOllama = useOllamaStore((s) => s.refresh);
  const [showOnboarding, setShowOnboarding] = React.useState<boolean | null>(null);
  const [section, setSection] = React.useState<Section>("general");

  React.useEffect(() => {
    void hydrate();
  }, [hydrate]);

  // Probe Ollama on launch and whenever the window regains focus, so the app
  // reflects a running daemon / pulled model without the manual Refresh button.
  // The tray re-shows this window rather than recreating it, so React never
  // remounts — the focus listener is what keeps state fresh after hide/show.
  React.useEffect(() => {
    void refreshOllama();
    let unlisten: (() => void) | null = null;
    let disposed = false;
    (async () => {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const un = await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
        if (focused) void refreshOllama();
      });
      if (disposed) un();
      else unlisten = un;
    })().catch(() => {
      // Not in a Tauri webview (vite dev) — DOM focus is good enough there
      const onFocus = () => void refreshOllama();
      window.addEventListener("focus", onFocus);
      if (!disposed) unlisten = () => window.removeEventListener("focus", onFocus);
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refreshOllama]);

  React.useEffect(() => {
    if (hydrated) setShowOnboarding(!settings.onboardingComplete);
  }, [hydrated, settings.onboardingComplete]);

  // Global hotkey feedback — the Rust shortcut handler was previously silent (log only),
  // so pressing the hotkey looked like "nothing happens". Now we toast every event.
  React.useEffect(() => {
    let unlistenFns: (() => void)[] = [];
    (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const events: [string, (payload: unknown) => void][] = [
          ["quill://no-selection", () => toast.error("Select some text first, then press the hotkey.")],
          ["quill://needs-permission", () => toast.error("Accessibility permission needed — open Settings → Privacy & Security → Accessibility, toggle Quill on, then press the hotkey again (no restart needed).")],
          ["quill://error", (p) => {
            const msg = typeof p === "string" ? p : (p as { payload?: string })?.payload ?? String(p);
            // Don't double-toast empty-result which already has quill://error
            if (msg) toast.error(msg);
          }],
          ["quill://paste-failed", (p) => {
            const msg = typeof p === "string" ? p : (p as { payload?: string })?.payload ?? "";
            toast.warning(msg || "Paste didn't land — result left on clipboard, press Cmd+V manually.");
          }],
          ["quill://pasted", () => toast.success("Rewritten — pasted in place (also on clipboard).")],
          ["quill://warning", (p) => {
            const msg = typeof p === "string" ? p : (p as { payload?: string })?.payload ?? String(p);
            if (msg) toast(msg);
          }],
          ["quill://hotkey-error", (p) => {
            const msg = typeof p === "string" ? p : (p as { payload?: string })?.payload ?? String(p);
            toast.error(`Hotkey not registered: ${msg} — change it in Settings.`);
          }],
        ];
        for (const [ev, handler] of events) {
          // Tauri listen payload is { payload: T }
          const un = await listen(ev, (e: unknown) => {
            const payload = (e as { payload?: unknown })?.payload ?? e;
            (handler as (p: unknown) => void)(payload);
          });
          unlistenFns.push(un);
        }
      } catch {
        // Not in Tauri webview (vite dev without plugin) — ignore
      }
    })();
    return () => {
      for (const fn of unlistenFns) try { fn(); } catch {}
    };
  }, []);

  // allow manual toggle via query param for dev / debugging
  React.useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    if (params.get("view") === "onboarding") setShowOnboarding(true);
    if (params.get("view") === "settings") setShowOnboarding(false);
  }, []);

  if (!hydrated || showOnboarding === null) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-zinc-50 dark:bg-zinc-950">
        <div className="flex items-center gap-2 text-sm text-zinc-500">
          <Loader2 className="h-4 w-4 animate-spin" /> Loading Quill…
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-screen bg-zinc-50 text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100">
      {showOnboarding ? (
        <div className="flex-1">
          <Onboarding onFinished={() => setShowOnboarding(false)} />
        </div>
      ) : (
        <>
          <Sidebar current={section} onSelect={setSection} />
          <main className="min-w-0 flex-1 overflow-y-auto">
            <div className="mx-auto max-w-lg p-6">
              {section === "general" && <GeneralSettings />}
              {section === "model" && <ModelSettings />}
              {section === "history" && <HistorySettings />}
            </div>
          </main>
        </>
      )}

      {/* dev toggle */}
      {import.meta.env.DEV && (
        <div className="fixed bottom-3 right-3 flex gap-1 rounded-full border border-zinc-200 bg-white p-1 shadow dark:border-zinc-700 dark:bg-zinc-900">
          <button
            onClick={() => setShowOnboarding(true)}
            className={`rounded-full px-3 py-1 text-xs ${showOnboarding ? "bg-zinc-900 text-white dark:bg-white dark:text-zinc-900" : "text-zinc-600"}`}
          >
            Onboarding
          </button>
          <button
            onClick={() => setShowOnboarding(false)}
            className={`rounded-full px-3 py-1 text-xs ${!showOnboarding ? "bg-zinc-900 text-white dark:bg-white dark:text-zinc-900" : "text-zinc-600"}`}
          >
            Main window
          </button>
        </div>
      )}

      <Toaster richColors position="top-right" />
    </div>
  );
}
