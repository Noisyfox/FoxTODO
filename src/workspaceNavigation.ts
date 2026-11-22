import { useEffect, useEffectEvent, useRef, useState } from "react";
import { exit, onBackButtonPress } from "@tauri-apps/api/app";
import { report } from "./session";

type WorkspacePage = {
  active: string;
  task: { list: string; id: string } | null;
  nav: boolean;
  search: string;
};

const initial: WorkspacePage = {
  active: "day",
  task: null,
  nav: false,
  search: "",
};
const overlaySelector = [
  '[data-slot="dialog-content"][data-state="open"]',
  '[data-slot="dropdown-menu-content"][data-state="open"]',
  '[data-slot="popover-content"][data-state="open"]',
  '[data-slot="combobox-content"]:not([data-closed])',
].join(",");

export function hasOpenOverlay() {
  return !!document.querySelector(overlaySelector);
}

function dismissOverlay() {
  if (!hasOpenOverlay()) return false;
  // Let the focused shadcn control dismiss its own topmost layer.
  (document.activeElement || document.body).dispatchEvent(
    new KeyboardEvent("keydown", {
      key: "Escape",
      code: "Escape",
      bubbles: true,
      cancelable: true,
    }),
  );
  return true;
}

export function useWorkspaceNavigation(singlePage: boolean, android: boolean) {
  const [page, setPage] = useState<WorkspacePage>(
    () => history.state?.foxPage ?? initial,
  );
  const current = useRef(page);
  function show(change: Partial<WorkspacePage>, newPage = false) {
    const next = { ...current.current, ...change };
    const push = singlePage && newPage;
    const entry = {
      ...history.state,
      foxPage: next,
      foxHasBack: singlePage && (push || !!history.state?.foxHasBack),
    };
    if (push) history.pushState(entry, "");
    else history.replaceState(entry, "");
    current.current = next;
    setPage(next);
  }
  function closePage() {
    if (singlePage && history.state?.foxHasBack) history.back();
    else show({ nav: false, task: null });
  }
  useEffect(() => {
    if (!history.state?.foxPage)
      history.replaceState({ ...history.state, foxPage: current.current }, "");
    const restore = (event: PopStateEvent) => {
      if (hasOpenOverlay()) {
        history.pushState(
          { ...history.state, foxPage: current.current, foxHasBack: true },
          "",
        );
        dismissOverlay();
        return;
      }
      const next: WorkspacePage = event.state?.foxPage ?? initial;
      current.current = next;
      setPage(next);
    };
    window.addEventListener("popstate", restore);
    return () => window.removeEventListener("popstate", restore);
  }, []);

  const systemBack = useEffectEvent((canGoBack: boolean) => {
    if (dismissOverlay()) return;
    if (canGoBack && (history.state?.foxHasBack || (!page.task && !page.nav)))
      history.back();
    else if (page.nav || page.task) show({ nav: false, task: null });
    else void exit(0).catch(report);
  });
  useEffect(() => {
    if (!android) return;
    const listener = onBackButtonPress(({ canGoBack }) =>
      systemBack(canGoBack),
    );
    void listener.catch(report);
    return () => {
      void listener
        .then((subscription) => subscription.unregister())
        .catch(report);
    };
  }, [android]);
  return { page, show, closePage };
}
