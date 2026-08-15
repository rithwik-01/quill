import * as React from "react";
import { Sparkles } from "lucide-react";
import { toast } from "sonner";
import { OllamaCheck } from "./OllamaCheck";
import { ModelPick } from "./ModelPick";
import { PermissionStep } from "./PermissionStep";
import { useSettingsStore } from "../../stores/settingsStore";

type Step = "ollama" | "model" | "permissions" | "done";

export function Onboarding({ onFinished }: { onFinished?: () => void }) {
  const [step, setStep] = React.useState<Step>("ollama");
  const { setOnboardingComplete } = useSettingsStore();

  const finish = async () => {
    await setOnboardingComplete(true);
    toast.success("Quill is ready — select text in any app and press your hotkey (default Cmd+Shift+G)");
    onFinished?.();
  };

  return (
    <div className="mx-auto flex min-h-screen max-w-lg flex-col gap-6 p-6">
      <header className="flex items-center gap-3 pt-4">
        <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-zinc-900 text-white dark:bg-white dark:text-zinc-900">
          <Sparkles className="h-5 w-5" />
        </div>
        <div>
          <h1 className="text-lg font-semibold leading-none text-zinc-900 dark:text-white">Welcome to Quill</h1>
          <p className="text-sm text-zinc-500">Select text → hotkey → rewritten in place. Local, private.</p>
        </div>
      </header>

      <div className="flex items-center gap-2">
        {(["ollama", "model", "permissions"] as Step[]).map((s, i) => (
          <React.Fragment key={s}>
            <div
              className={`flex h-7 w-7 items-center justify-center rounded-full text-xs font-medium ${
                step === s
                  ? "bg-zinc-900 text-white dark:bg-white dark:text-zinc-900"
                  : ["ollama", "model", "permissions"].indexOf(step) > i
                    ? "bg-green-600 text-white"
                    : "bg-zinc-200 text-zinc-500 dark:bg-zinc-800"
              }`}
            >
              {i + 1}
            </div>
            {i < 2 && <div className="h-px flex-1 bg-zinc-200 dark:bg-zinc-800" />}
          </React.Fragment>
        ))}
      </div>

      <div className="rounded-2xl border border-zinc-200 bg-white p-5 shadow-sm dark:border-zinc-800 dark:bg-zinc-900">
        {step === "ollama" && <OllamaCheck onReady={() => setStep("model")} />}
        {step === "model" && <ModelPick onComplete={() => setStep("permissions")} />}
        {/* Mounted only on this step so the permission poll doesn't run during
            the earlier ones. */}
        {step === "permissions" && <PermissionStep onFinish={() => void finish()} />}
      </div>

      {step !== "permissions" && step !== "done" && (
        <p className="text-center text-xs text-zinc-400">
          {step === "ollama" ? "Step 1 of 3 — Ollama" : "Step 2 of 3 — Model"}
        </p>
      )}
    </div>
  );
}
