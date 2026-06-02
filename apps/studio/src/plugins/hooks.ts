// apps/studio/src/plugins/hooks.ts

import { useEffect, useSyncExternalStore } from "react";
import type { PluginHost } from "./PluginHost";
import type { ToolEntry, ToolContext } from "./types";

/**
 * React hook that subscribes to tool changes and returns the active tool.
 * Also activates the tool's `onActivate` lifecycle when context is ready.
 */
export function useActiveTool(
  host: PluginHost,
  context: ToolContext | null,
): ToolEntry | null {
  const activeTool = useSyncExternalStore(
    (onStoreChange) => host.on("toolChanged", onStoreChange),
    () => host.getActiveTool(),
  );

  useEffect(() => {
    if (activeTool && context) {
      host.notifyToolActivated(context);
    }
  }, [activeTool, context, host]);

  return activeTool;
}

/** Hook returning the list of all registered tools */
export function usePluginTools(host: PluginHost): ToolEntry[] {
  return useSyncExternalStore(
    (onStoreChange) => host.on("registered", onStoreChange),
    () => host.getTools(),
  );
}
