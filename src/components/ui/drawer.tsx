import { createContext, useCallback, useContext, useEffect, useId, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const DrawerPortalContext = createContext<HTMLElement | null>(null);

// Keep portaled popovers within the drawer's focus scope and stacking context.
export function useDrawerPortalContainer() {
  return useContext(DrawerPortalContext);
}

const FOCUSABLE = "button:not([disabled]), a[href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])";

export interface DrawerProps {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: ReactNode;
  headerContent?: ReactNode;
  footer?: ReactNode;
  children: ReactNode;
  /** Preferred width in pixels; automatically fits narrower viewports. */
  width?: number;
  closeDisabled?: boolean;
  /** Let a companion document viewer own focus and Escape while it is open. */
  suspendFocusManagement?: boolean;
  closeLabel?: string;
  bodyClassName?: string;
  footerClassName?: string;
}

export function Drawer({
  open, onClose, title, description, headerContent, footer, children,
  width = 900, closeDisabled = false, suspendFocusManagement = false,
  closeLabel = "Close drawer", bodyClassName, footerClassName,
}: DrawerProps) {
  const titleId = useId();
  const descriptionId = useId();
  const rootRef = useRef<HTMLDivElement | null>(null);
  const panelRef = useRef<HTMLElement | null>(null);
  const closeRef = useRef<HTMLButtonElement | null>(null);
  const [portalContainer, setPortalContainer] = useState<HTMLElement | null>(null);
  const attachRoot = useCallback((element: HTMLDivElement | null) => {
    rootRef.current = element;
    setPortalContainer(element);
  }, []);

  useEffect(() => {
    if (!open) return;
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previousOverflow;
      if (previousFocus?.isConnected) previousFocus.focus();
    };
  }, [open]);

  useEffect(() => {
    if (!open || suspendFocusManagement) return;
    const root = rootRef.current;
    const panel = panelRef.current;
    if (!root || !panel) return;
    const focusDrawer = () => {
      const initial = panel.querySelector<HTMLElement>("[data-drawer-autofocus]:not([disabled])");
      (initial ?? (closeRef.current?.disabled ? null : closeRef.current) ?? panel).focus({ preventScroll: true });
    };
    const focusFrame = requestAnimationFrame(() => {
      if (!root.contains(document.activeElement)) focusDrawer();
    });
    const onFocusIn = (event: FocusEvent) => {
      if (event.target instanceof Node && !root.contains(event.target)) focusDrawer();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      if (event.key === "Escape") {
        event.preventDefault();
        if (!closeDisabled) onClose();
        return;
      }
      if (event.key !== "Tab") return;
      const focusable = Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE))
        .filter((element) => element.tabIndex >= 0 && element.getClientRects().length > 0);
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (!first) {
        event.preventDefault();
        panel.focus();
      } else if (!focusable.includes(document.activeElement as HTMLElement)) {
        event.preventDefault();
        (event.shiftKey ? last : first).focus();
      } else if (event.shiftKey && (document.activeElement === first || !root.contains(document.activeElement))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("focusin", onFocusIn, true);
    // Bubble phase lets nested popovers consume Escape first.
    window.addEventListener("keydown", onKeyDown);
    return () => {
      cancelAnimationFrame(focusFrame);
      document.removeEventListener("focusin", onFocusIn, true);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [open, onClose, closeDisabled, suspendFocusManagement]);

  if (!open) return null;
  return createPortal(
    <DrawerPortalContext.Provider value={portalContainer}>
      <div ref={attachRoot} className="fixed inset-0 z-50 overflow-hidden">
        <button type="button" tabIndex={-1} aria-label={closeLabel} disabled={closeDisabled}
          onClick={onClose} className="absolute inset-0 bg-overlay animate-drawer-scrim-in" />
        <aside ref={panelRef} role="dialog" aria-modal={suspendFocusManagement ? undefined : true}
          aria-labelledby={titleId} aria-describedby={description ? descriptionId : undefined} tabIndex={-1}
          style={{ width: `min(100%, ${width}px)` }}
          className="absolute right-0 top-0 flex h-full max-w-full flex-col border-l border-border bg-card text-foreground shadow-pop animate-drawer-slide-in">
          <header className="shrink-0 border-b border-border px-5 py-4">
            <div className="flex items-start justify-between gap-4">
              <h2 id={titleId} className="font-head text-lg font-semibold">{title}</h2>
              <Button ref={closeRef} variant="ghost" size="icon" onClick={onClose} disabled={closeDisabled} aria-label={closeLabel}><X className="size-4" /></Button>
            </div>
            {description && <p id={descriptionId} className="mt-2 text-sm text-muted-foreground">{description}</p>}
            {headerContent}
          </header>
          <div className={cn("min-h-0 min-w-0 flex-1 overflow-y-auto px-5 py-4", bodyClassName)}>{children}</div>
          {footer && <footer className={cn("flex shrink-0 flex-wrap items-center justify-between gap-3 border-t border-border px-5 py-4", footerClassName)}>{footer}</footer>}
        </aside>
      </div>
    </DrawerPortalContext.Provider>,
    document.body,
  );
}
