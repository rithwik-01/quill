import * as React from "react";
import { commands, type AccessibilityStatus } from "../bindings";

/**
 * Single source of truth for the macOS Accessibility permission.
 *
 * Apple gives no change notification for this, so the supported pattern is:
 * call `AXIsProcessTrustedWithOptions(prompt:)` once, then poll
 * `AXIsProcessTrusted()` until it flips. That's all this does — one interval,
 * running only while untrusted, stopped the moment it's granted.
 *
 * `checking` is deliberately NOT set by the background poll. Wiring a 1s poll
 * to a button's `disabled`/`loading` prop makes the whole UI flicker once a
 * second, which is what made this screen feel broken.
 */

const POLL_MS = 1000;

export type UseAccessibility = {
  /** `null` until the first check resolves. */
  trusted: boolean | null;
  /** True only during an explicit `refresh()` — never during background polls. */
  checking: boolean;
  /** Set when the check itself failed. Never reported as granted. */
  error: string | null;
  /** Diagnostics for the stale-entry help text; loaded lazily by `loadStatus`. */
  status: AccessibilityStatus | null;
  /** Shows the native prompt if this app identity has never been asked. */
  request: () => Promise<void>;
  /** Deep-links to Privacy & Security → Accessibility. */
  openSettings: () => Promise<void>;
  /** Explicit user-triggered re-check (sets `checking`). */
  refresh: () => Promise<boolean>;
  loadStatus: () => Promise<void>;
};

export function useAccessibility(): UseAccessibility {
  const [trusted, setTrusted] = React.useState<boolean | null>(null);
  const [checking, setChecking] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [status, setStatus] = React.useState<AccessibilityStatus | null>(null);

  const check = React.useCallback(async (): Promise<boolean> => {
    try {
      const ok = await commands.isAccessibilityTrusted();
      setTrusted(ok);
      setError(null);
      return ok;
    } catch (e) {
      // Never claim granted on a failed check — that hid real failures before.
      setTrusted(false);
      setError(e instanceof Error ? e.message : String(e));
      return false;
    }
  }, []);

  // `trusted !== true` rather than `trusted` as the dep: null → false must not
  // tear down and rebuild the interval, only the flip to granted should.
  const shouldPoll = trusted !== true;

  // Poll while untrusted; stop as soon as it's granted.
  React.useEffect(() => {
    void check();
    if (!shouldPoll) return;
    const id = setInterval(() => void check(), POLL_MS);
    return () => clearInterval(id);
  }, [check, shouldPoll]);

  // Re-check when the user comes back from System Settings.
  React.useEffect(() => {
    if (!shouldPoll) return;
    let unlisten: (() => void) | undefined;
    void (async () => {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        unlisten = await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
          if (focused) void check();
        });
      } catch {
        // not running under Tauri (plain vite dev) — the poll still covers it
      }
    })();
    return () => unlisten?.();
  }, [check, shouldPoll]);

  const request = React.useCallback(async () => {
    try {
      // Returns the CURRENT trust, not the post-dialog state (the prompt is
      // async), so we just let the poll pick up the change.
      await commands.requestAccessibilityPermission();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
    void check();
  }, [check]);

  const openSettings = React.useCallback(async () => {
    try {
      await commands.openAccessibilitySettings();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const refresh = React.useCallback(async () => {
    setChecking(true);
    try {
      return await check();
    } finally {
      setChecking(false);
    }
  }, [check]);

  const loadStatus = React.useCallback(async () => {
    try {
      const res = await commands.getAccessibilityStatus();
      if (res.status === "ok") setStatus(res.data);
    } catch {
      // diagnostics are best-effort
    }
  }, []);

  return { trusted, checking, error, status, request, openSettings, refresh, loadStatus };
}
