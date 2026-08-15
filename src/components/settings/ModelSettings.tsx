import * as React from "react";
import { Cpu, RefreshCw, Download, CheckCircle2, X, MemoryStick } from "lucide-react";
import { toast } from "sonner";
import { Select } from "../ui/Select";
import { Button } from "../ui/Button";
import { ProgressBar } from "../ui/ProgressBar";
import { useSettingsStore } from "../../stores/settingsStore";
import { MODEL_OPTIONS, hasModel, useOllamaStore } from "../../stores/ollamaStore";
import { commands } from "../../bindings";

export function ModelSettings() {
  const { settings, setModel } = useSettingsStore();
  const {
    refresh,
    installedModels,
    recommendedModel,
    checked,
    isAvailable,
    pullProgress,
    status,
    pullModel,
    cancelPull,
  } = useOllamaStore();
  const [checkingModels, setCheckingModels] = React.useState(false);
  const [systemRam, setSystemRam] = React.useState<number | null>(null);

  React.useEffect(() => {
    void (async () => {
      try {
        setSystemRam(await commands.getSystemRamGb());
      } catch {
        // non-Tauri dev context
      }
    })();
  }, []);

  const handleRefreshModels = async () => {
    setCheckingModels(true);
    try {
      const ok = await refresh();
      if (ok) toast.success("Model list refreshed");
      else toast.error("Could not reach Ollama");
    } finally {
      setCheckingModels(false);
    }
  };

  const handleModelChange = async (v: string) => {
    try {
      await setModel(v);
      toast.success(`Model set to ${v}`);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Failed to save model");
    }
  };

  const isPulling = status === "pulling";
  const installed = hasModel(installedModels, settings.model);

  const handlePull = async () => {
    try {
      await pullModel(settings.model);
      await refresh();
      toast.success(`${settings.model} ready`);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Download failed");
    }
  };

  return (
    <div className="space-y-6">
      <header>
        <h1 className="text-lg font-semibold text-zinc-900 dark:text-white">Model</h1>
        <p className="text-sm text-zinc-500">
          One local model powers all four actions and the refine chat.
        </p>
      </header>

      <section className="rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900">
        <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold text-zinc-900 dark:text-white">
          <Cpu className="h-4 w-4" /> Active model
        </h2>
        <p className="mb-3 text-sm text-zinc-600 dark:text-zinc-400">
          Recommended: <span className="font-medium text-zinc-900 dark:text-white">{recommendedModel}</span>
          {systemRam ? (
            <span className="ml-2 inline-flex items-center gap-1 text-xs text-zinc-400">
              <MemoryStick className="h-3 w-3" /> {Math.round(systemRam)} GB RAM
            </span>
          ) : null}
        </p>
        <div className="flex items-center gap-2">
          <div className="flex-1">
            <Select
              value={settings.model}
              onValueChange={(v) => void handleModelChange(v)}
              options={MODEL_OPTIONS.map((o) => ({
                value: o.value,
                label: o.label,
                hint: hasModel(installedModels, o.value) ? "installed" : undefined,
              }))}
              disabled={isPulling}
            />
          </div>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => void handleRefreshModels()}
            loading={checkingModels}
            title="Refresh from GET /api/tags"
          >
            <RefreshCw className="h-4 w-4" />
          </Button>
        </div>

        {!checked ? (
          <p className="mt-2 text-xs text-zinc-500">Checking Ollama…</p>
        ) : !isAvailable ? (
          <p className="mt-2 text-xs text-amber-600">Ollama isn't running — start it, then hit refresh.</p>
        ) : installed ? (
          <p className="mt-2 flex items-center gap-1.5 text-xs text-green-700 dark:text-green-400">
            <CheckCircle2 className="h-3.5 w-3.5" /> {settings.model} is downloaded and active
          </p>
        ) : (
          <div className="mt-3 flex items-center gap-2">
            <Button variant="primary" size="sm" onClick={() => void handlePull()} loading={isPulling}>
              <Download className="h-4 w-4" /> Download {settings.model}
            </Button>
            {isPulling && (
              <Button variant="ghost" size="sm" onClick={cancelPull} title="Cancel download">
                <X className="h-4 w-4" />
              </Button>
            )}
          </div>
        )}

        {isPulling && pullProgress && (
          <div className="mt-4 rounded-xl border border-zinc-200 p-4 dark:border-zinc-800">
            <div className="mb-2 flex items-center justify-between text-sm">
              <span className="font-medium text-zinc-700 dark:text-zinc-300">
                Downloading {settings.model}
              </span>
              <span className="text-xs text-zinc-400">{pullProgress.percent}%</span>
            </div>
            <ProgressBar value={pullProgress.percent} showLabel />
            <p className="mt-2 truncate text-xs text-zinc-500">{pullProgress.status}</p>
            <p className="mt-1 text-xs text-zinc-400">
              {pullProgress.completed > 0 && pullProgress.total > 0
                ? `${(pullProgress.completed / 1e9).toFixed(2)} / ${(pullProgress.total / 1e9).toFixed(2)} GB`
                : "Starting…"}
            </p>
          </div>
        )}
      </section>
    </div>
  );
}
