import * as React from "react";
import { CheckCircle2, AlertTriangle, Loader2, ExternalLink, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "../ui/Button";
import { useOllamaStore } from "../../stores/ollamaStore";
import { commands } from "../../bindings";

type Props = {
  onReady: () => void;
};

export function OllamaCheck({ onReady }: Props) {
  const { status, isAvailable, error, checkOllama } = useOllamaStore();
  const [ran, setRan] = React.useState(false);

  const runCheck = React.useCallback(async () => {
    // PLAN §10: liveness probe is GET /api/version with 1.5s timeout
    const ok = await checkOllama();
    if (ok) toast.success("Ollama is running");
    return ok;
  }, [checkOllama]);

  React.useEffect(() => {
    if (!ran) {
      setRan(true);
      void runCheck();
    }
  }, [ran, runCheck]);

  const handleRetry = async () => {
    const ok = await runCheck();
    if (ok) onReady();
    else toast.error("Ollama still not running");
  };

  const handleContinue = async () => {
    const ok = await runCheck();
    if (ok) onReady();
  };

  const openOllamaSite = async () => {
    try {
      await (commands as unknown as { openUrl: (u: string) => Promise<void> }).openUrl("https://ollama.com");
    } catch {
      window.open("https://ollama.com", "_blank");
    }
  };

  if (status === "checking" && !ran) {
    return (
      <div className="flex flex-col items-center gap-3 py-8 text-center">
        <Loader2 className="h-8 w-8 animate-spin text-zinc-400" />
        <p className="text-sm text-zinc-600 dark:text-zinc-400">Checking for Ollama… (GET /api/version, 1.5s timeout)</p>
      </div>
    );
  }

  if (status === "checking") {
    return (
      <div className="flex flex-col items-center gap-3 py-8">
        <Loader2 className="h-8 w-8 animate-spin text-zinc-400" />
        <p className="text-sm text-zinc-600 dark:text-zinc-400">Checking Ollama…</p>
      </div>
    );
  }

  if (status === "missing" || !isAvailable) {
    return (
      <div className="flex flex-col gap-6 py-4">
        <div className="flex gap-3 rounded-xl border border-amber-200 bg-amber-50 p-4 dark:border-amber-900 dark:bg-amber-950/30">
          <AlertTriangle className="h-5 w-5 shrink-0 text-amber-600" />
          <div className="space-y-1">
            <p className="text-sm font-medium text-amber-900 dark:text-amber-100">Ollama isn't running.</p>
            <p className="text-sm text-amber-800 dark:text-amber-200">
              Quill needs Ollama to run local models. Install it, then come back and hit Retry.
            </p>
            {error && <p className="text-xs text-amber-700 dark:text-amber-300">{error}</p>}
          </div>
        </div>

        <ol className="list-decimal space-y-2 pl-5 text-sm text-zinc-600 dark:text-zinc-400">
          <li>Download Ollama from ollama.com</li>
          <li>Open the installer and start Ollama</li>
          <li>Return here and press Retry</li>
        </ol>

        <div className="flex gap-2">
          <Button onClick={openOllamaSite} variant="secondary">
            <ExternalLink className="h-4 w-4" /> Install Ollama
          </Button>
          <Button onClick={handleRetry} variant="primary">
            <RefreshCw className="h-4 w-4" /> Retry
          </Button>
        </div>

        <p className="text-xs text-zinc-500">Probe: GET http://127.0.0.1:11434/api/version — 1.5s timeout.</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4 py-4">
      <div className="flex items-center gap-2 rounded-lg bg-green-50 px-3 py-2 text-green-800 dark:bg-green-950/30 dark:text-green-300">
        <CheckCircle2 className="h-5 w-5" />
        <span className="text-sm font-medium">Ollama is running</span>
      </div>
      <p className="text-sm text-zinc-600 dark:text-zinc-400">We found Ollama via GET /api/version. Next, pick a model.</p>
      <Button onClick={handleContinue} variant="primary" className="self-start">
        Continue
      </Button>
    </div>
  );
}
