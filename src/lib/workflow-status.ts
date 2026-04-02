const WORKFLOW_EVENT = "workflow-status-changed";

export function notifyWorkflowStatusChanged() {
  if (typeof window === "undefined") return;
  window.dispatchEvent(new Event(WORKFLOW_EVENT));
}

export function subscribeWorkflowStatusChanged(listener: () => void) {
  if (typeof window === "undefined") return () => {};
  window.addEventListener(WORKFLOW_EVENT, listener);
  return () => window.removeEventListener(WORKFLOW_EVENT, listener);
}

export { WORKFLOW_EVENT };
