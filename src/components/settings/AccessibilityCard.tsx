import * as React from "react";
import { ShieldCheck, RefreshCw } from "lucide-react";
import { Button } from "../ui/Button";
import { useAccessibility } from "../../hooks/useAccessibility";

/**
 * Post-onboarding surface for the Accessibility permission. Without this there
 * is no way back for someone who hit "Skip for now", or whose grant stopped
 * matching after the app's code signature changed.
 */
export function AccessibilityCard() {
  const { trusted, checking, error, status, request, openSettings, refresh, loadStatus } =
    useAccessibility();

  React.useEffect(() => {
    if (trusted === false && !status) void loadStatus();
  }, [trusted, status, loadStatus]);

  return (
    <section className="rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900">
      <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold text-zinc-900 dark:text-white">
        <ShieldCheck className="h-4 w-4" /> Accessibility
      </h2>
      <p className="mb-3 text-sm text-zinc-600 dark:text-zinc-400">
        Required to read your selection and paste the result back.
      </p>

      {trusted === null ? (
        <p className="text-xs text-zinc-500">Checking…</p>
      ) : trusted ? (
        <p className="text-sm text-green-700 dark:text-green-400">Granted.</p>
      ) : (
        <>
          <p className="text-sm text-amber-600 dark:text-amber-400">
            Not granted — the hotkey won't do anything until you turn this on.
          </p>
          {status && (
            <p className="mt-2 text-xs text-zinc-500">
              If System Settings already shows a Quill toggle that's on, it belongs to an older
              copy: remove every Quill entry with <strong>−</strong> and grant again, or run{" "}
              <code className="font-mono">tccutil reset Accessibility {status.bundleIdentifier}</code>.
              macOS keeps these entries even after an app is deleted — that's by design.
            </p>
          )}
          <div className="mt-3 flex flex-wrap gap-2">
            <Button variant="primary" size="sm" onClick={() => void request()}>
              Grant Access
            </Button>
            <Button variant="secondary" size="sm" onClick={() => void openSettings()}>
              Open System Settings
            </Button>
            <Button variant="secondary" size="sm" onClick={() => void refresh()} loading={checking}>
              <RefreshCw className="h-4 w-4" />
            </Button>
          </div>
        </>
      )}

      {error && <p className="mt-2 text-xs text-red-600">Couldn't check the permission: {error}</p>}
    </section>
  );
}
