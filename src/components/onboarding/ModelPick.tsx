import * as React from "react";
import { Download, CheckCircle2, Loader2, AlertCircle, X } from "lucide-react";
import { toast } from "sonner";
import { Button } from "../ui/Button";
import { Select } from "../ui/Select";
import { ProgressBar } from "../ui/ProgressBar";
import { MODEL_OPTIONS, hasModel, useOllamaStore } from "../../stores/ollamaStore";
import { useSettingsStore } from "../../stores/settingsStore";

type Props = {
  onComplete: () => void;
};

export function ModelPick({ onComplete }: Props) {
  const { selectedModel, recommendedModel, installedModels, pullProgress, status, fetchTags, pullModel, cancelPull, setSelectedModel } = useOllamaStore();
  const { setModel } = useSettingsStore();
  const [loadingTags, setLoadingTags] = React.useState(true);

  React.useEffect(() => {
    let alive = true;
    (async () => {
      // GET /api/tags — discover already-pulled models
      setLoadingTags(true);
      const tags = await fetchTags();
      if (!alive) return;
      // Read the current selection at run time instead of via deps, so changing
      // the dropdown doesn't re-trigger this effect and refetch tags.
      const { selectedModel: sel, recommendedModel: rec } = useOllamaStore.getState();
      // prefer recommended if already present, else keep selection
      if (tags.length > 0 && !hasModel(tags, sel) && hasModel(tags, rec)) {
        setSelectedModel(rec);
      }
      setLoadingTags(false);
    })();
    return () => {
      alive = false;
    };
  }, [fetchTags, setSelectedModel]);

  const isInstalled = hasModel(installedModels, selectedModel);
  const isPulling = status === "pulling";
  const pullPct = pullProgress?.percent ?? 0;

  const handlePull = async () => {
    try {
      // POST /api/pull streaming NDJSON — {status, digest, total, completed} per line
      await pullModel(selectedModel);
      await setModel(selectedModel);
      toast.success(`${selectedModel} ready`);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Download failed");
    }
  };

  const handleUseInstalled = async () => {
    await setModel(selectedModel);
    toast.success(`Using ${selectedModel}`);
    onComplete();
  };

  const handleContinueAfterPull = async () => {
    await setModel(selectedModel);
    onComplete();
  };

  // show success state after pull
  const justPulled = status === "ready" && isInstalled;

  return (
    <div className="flex flex-col gap-5 py-4">
      <div>
        <h3 className="text-sm font-semibold text-zinc-900 dark:text-zinc-100">Pick a model</h3>
        <p className="mt-1 text-sm text-zinc-600 dark:text-zinc-400">
          Recommended for your machine: <span className="font-medium text-zinc-900 dark:text-white">{recommendedModel}</span>. You can override.
        </p>
      </div>

      <Select
        value={selectedModel}
        onValueChange={setSelectedModel}
        options={MODEL_OPTIONS.map((o) => ({
          value: o.value,
          label: o.label,
          hint: o.ram + (o.value === recommendedModel ? " · recommended" : ""),
        }))}
        disabled={isPulling}
      />

      {loadingTags ? (
        <div className="flex items-center gap-2 text-sm text-zinc-500">
          <Loader2 className="h-4 w-4 animate-spin" /> Checking installed models (GET /api/tags)…
        </div>
      ) : isInstalled ? (
        <div className="flex items-center gap-2 rounded-lg bg-green-50 px-3 py-2 text-sm text-green-800 dark:bg-green-950/30 dark:text-green-300">
          <CheckCircle2 className="h-4 w-4" /> {selectedModel} is already downloaded
        </div>
      ) : (
        <div className="flex items-center gap-2 rounded-lg bg-zinc-100 px-3 py-2 text-sm text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400">
          <AlertCircle className="h-4 w-4" /> {selectedModel} not downloaded yet — about{" "}
          {MODEL_OPTIONS.find((o) => o.value === selectedModel)?.label.match(/\(([^)]+)\)/)?.[1] ?? "a few GB"}
        </div>
      )}

      {isPulling && pullProgress && (
        <div className="rounded-xl border border-zinc-200 p-4 dark:border-zinc-800">
          <div className="mb-2 flex items-center justify-between">
            <span className="text-sm font-medium text-zinc-700 dark:text-zinc-300">Downloading {selectedModel}</span>
            <button onClick={cancelPull} className="rounded p-1 hover:bg-zinc-100 dark:hover:bg-zinc-800">
              <X className="h-4 w-4" />
            </button>
          </div>
          <ProgressBar value={pullPct} showLabel />
          <p className="mt-2 truncate text-xs text-zinc-500">
            {pullProgress.status}
            {pullProgress.digest ? ` · ${pullProgress.digest.slice(0, 12)}` : ""} — NDJSON stream
          </p>
          <p className="mt-1 text-xs text-zinc-400">
            {pullProgress.completed > 0 && pullProgress.total > 0
              ? `${(pullProgress.completed / 1e9).toFixed(2)} / ${(pullProgress.total / 1e9).toFixed(2)} GB`
              : "Starting…"}
          </p>
        </div>
      )}

      {justPulled && !isPulling && (
        <div className="flex items-center gap-2 rounded-lg bg-green-50 px-3 py-2 text-sm text-green-800 dark:bg-green-950/30 dark:text-green-300">
          <CheckCircle2 className="h-4 w-4" /> Download complete
        </div>
      )}

      <div className="flex gap-2">
        {!isInstalled ? (
          <Button onClick={handlePull} loading={isPulling} disabled={loadingTags || isPulling}>
            <Download className="h-4 w-4" /> Download {selectedModel}
          </Button>
        ) : (
          <Button onClick={handleUseInstalled} variant="primary">
            Continue with {selectedModel}
          </Button>
        )}
        {justPulled && (
          <Button onClick={handleContinueAfterPull} variant="primary">
            Continue
          </Button>
        )}
      </div>

      <p className="text-xs text-zinc-500">POST /api/pull streams NDJSON progress — parsed {`{status, digest, total, completed}`} per line.</p>
    </div>
  );
}
