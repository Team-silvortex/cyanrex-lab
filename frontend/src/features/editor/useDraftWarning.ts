import { useEffect, useRef } from "react";

/** Browser exit and explicit in-page links warn; this is not durable draft recovery. */
export function useDraftWarning(dirty: boolean, message: string) {
  const current = useRef({ dirty, message }); current.current = { dirty, message };
  useEffect(() => {
    const unload = (event: BeforeUnloadEvent) => {
      if (current.current.dirty) { event.preventDefault(); event.returnValue = ""; }
    };
    const leave = (event: MouseEvent) => {
      if (!current.current.dirty || event.defaultPrevented || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
      const link = event.target instanceof Element ? event.target.closest<HTMLAnchorElement>("a[href]") : null;
      if (!link || link.download || (link.target && link.target !== "_self")) return;
      const target = new URL(link.href, location.href);
      if (target.origin === location.origin && target.pathname === location.pathname && target.search === location.search) return;
      if (!window.confirm(current.current.message)) { event.preventDefault(); event.stopPropagation(); }
    };
    window.addEventListener("beforeunload", unload);
    document.addEventListener("click", leave, true);
    return () => { window.removeEventListener("beforeunload", unload); document.removeEventListener("click", leave, true); };
  }, []);
}
