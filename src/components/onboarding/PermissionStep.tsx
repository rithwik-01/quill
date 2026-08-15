import * as React from "react";
import { ShieldCheck, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "../ui/Button";
import { useAccessibility } from "../../hooks/useAccessibility";

/** Delay before offering the System Settings deep link. The native prompt gets
 *  first crack at it, uninterrupted — opening both at once made Settings slam
 *  open on top of the dialog. */
const OFFER_SETTINGS_MS = 2000;
/** Delay before showing the stale-TCC-entry help. */
const OFFER_HELP_MS = 15000;

export function PermissionStep({ onFinish }: { onFinish: () => void }) {
  const { trusted, checking, error, status, request, openSettings, refresh, loadStatus } =
    useAccessibility();
  /** Set once the user has asked — drives the escalating help below. */
  const [requestedAt, setRequestedAt] = React.useState<number | null>(null);
  const [elapsed, setElapsed] = React.useState(0);

  // Toast once on the granted transition, not on every poll tick.
  const wasTrusted = React.useRef<boolean | null>(null);
  React.useEffect(() => {
    if (trusted === true && wasTrusted.current === false) {
      toast.success("Accessibility granted — you're all set.");
    }
    wasTrusted.current = trusted;
  }, [trusted]);

  React.useEffect(() => {
    if (requestedAt === null || trusted === true) return;
    const id = setInterval(() => setElapsed(Date.now() - requestedAt), 500);
    return () => clearInterval(id);
  }, [requestedAt, trusted]);

  const waiting = requestedAt !== null && trusted !== true;
  const showSettingsButton = waiting && elapsed > OFFER_SETTINGS_MS;
  const showStuckHelp = waiting && elapsed > OFFER_HELP_MS && trusted === false;

  React.useEffect(() => {
    if (showStuckHelp && !status) void loadStatus();
  }, [showStuckHelp, status, loadStatus]);

  const handleRequest = async () => {
    setRequestedAt(Date.now());
    setElapsed(0);
    await request();
  };

  return (
    <div className="flex flex-col gap-4 py-2">
      <div className="flex gap-3">
        <ShieldCheck className="h-5 w-5 shrink-0 text-zinc-700 dark:text-zinc-300" />
        <div className="space-y-1">
          <h3 className="text-sm font-semibold text-zinc-900 dark:text-white">Accessibility permission (macOS)</h3>
          <p className="text-sm text-zinc-600 dark:text-zinc-400">
            Quill needs Accessibility access to read your selected text and paste the result back.
          </p>
          <p className="text-xs text-zinc-500">Toggle Quill on in System Settings and return here — Quill picks it up automatically, no restart.</p>
          <p className="text-xs text-zinc-500">Quill leaves its result on your clipboard, so nothing is lost if the paste doesn't land.</p>
        </div>
      </div>

      {trusted === true && (
        <div className="rounded-lg bg-green-50 px-3 py-2 text-sm text-green-800 dark:bg-green-950/30 dark:text-green-300">
          Accessibility granted — you're all set.
        </div>
      )}
      {trusted === false && !waiting && (
        <div className="rounded-lg bg-amber-50 px-3 py-2 text-sm text-amber-800 dark:bg-amber-950/30 dark:text-amber-300">
          Not granted yet. Click Grant Accessibility Access below — macOS will ask for your approval.
        </div>
      )}
      {waiting && (
        <div className="flex items-center gap-2 rounded-lg bg-blue-50 px-3 py-2 text-sm text-blue-800 dark:bg-blue-950/30 dark:text-blue-300">
          <RefreshCw className="h-4 w-4 animate-spin" />
          Waiting for permission… Toggle Quill on under Privacy &amp; Security → Accessibility.
        </div>
      )}
      {error && (
        <div className="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-800 dark:bg-red-950/30 dark:text-red-300">
          Couldn't check the permission: {error}
        </div>
      )}

      {showStuckHelp && (
        <div className="rounded-lg border border-amber-200 bg-amber-50 p-3 text-xs text-amber-900 dark:border-amber-900 dark:bg-amber-950/30 dark:text-amber-200">
          <p className="mb-1 text-sm font-semibold">Still not detected? macOS likely has a stale permission entry.</p>
          <p className="mb-1">
            macOS ties the grant to the app's code signature. If that changed, an older "Quill" entry
            in System Settings no longer matches this copy. Fix: under Privacy &amp; Security →
            Accessibility, select <em>every</em> Quill entry, remove it with the <strong>−</strong>{" "}
            button, then grant again.
          </p>
          {status && !status.bundled && (
            <p className="mb-1">You're running the dev binary, not Quill.app — that's a separate entry and needs its own grant:</p>
          )}
          {status && <p className="mb-1 break-all font-mono">{status.executablePath}</p>}
          {status && (
            <p className="text-amber-700 dark:text-amber-300">
              Terminal alternative: <code className="font-mono">tccutil reset Accessibility {status.bundleIdentifier}</code>
            </p>
          )}
        </div>
      )}

      <div className="flex flex-wrap gap-2">
        {trusted !== true && (
          <Button onClick={handleRequest} variant="primary">
            Grant Accessibility Access
          </Button>
        )}
        {showSettingsButton && (
          <Button onClick={() => void openSettings()} variant="secondary">
            Open System Settings
          </Button>
        )}
        <Button onClick={() => void refresh()} variant="secondary" loading={checking}>
          <RefreshCw className="h-4 w-4" /> Retry
        </Button>
        <Button onClick={onFinish} variant={trusted === true ? "primary" : "ghost"}>
          {trusted === true ? "Finish" : "Skip for now"}
        </Button>
      </div>
    </div>
  );
}
