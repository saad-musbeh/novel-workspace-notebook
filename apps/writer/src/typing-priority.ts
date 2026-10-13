/** Cooperative window-local priority for writing. Never defer saves or drains. */
const QUIET_MS = 150;
const activity = new Map<object, { pending: boolean; composing: boolean; lastInput: number }>();
export function noteTyping(source: object): void {
  const current = activity.get(source) ?? { pending: false, composing: false, lastInput: 0 };
  current.lastInput = performance.now(); activity.set(source, current);
}
export function setTypingPending(source: object, pending: boolean): void {
  const current = activity.get(source) ?? { pending: false, composing: false, lastInput: -Infinity };
  current.pending = pending; activity.set(source, current);
}
export function setTypingComposing(source: object, composing: boolean): void {
  const current = activity.get(source) ?? { pending: false, composing: false, lastInput: -Infinity };
  current.composing = composing; activity.set(source, current);
}
export function releaseTyping(source: object): void { activity.delete(source); }
export function typingIsActive(): boolean {
  const now = performance.now();
  for (const entry of activity.values()) if (entry.pending || entry.composing || now - entry.lastInput < QUIET_MS) return true;
  return false;
}
/** Abortable checkpoint before background compute/reads; running work is not preempted. */
export async function waitForTypingIdle(signal?: AbortSignal): Promise<void> {
  for (;;) {
    while (typingIsActive()) {
      if (signal?.aborted) throw signal.reason ?? new DOMException('Cancelled', 'AbortError');
      await new Promise<void>((resolve, reject) => {
        const abort = () => { clearTimeout(timer); reject(signal?.reason ?? new DOMException('Cancelled', 'AbortError')); };
        const timer = setTimeout(() => { signal?.removeEventListener('abort', abort); resolve(); }, 16);
        signal?.addEventListener('abort', abort, { once: true });
      });
    }
    if (signal?.aborted) throw signal.reason ?? new DOMException('Cancelled', 'AbortError');
    // Even an idle caller yields one browser turn, then rechecks fresh input.
    await new Promise<void>(resolve => setTimeout(resolve, 0));
    if (signal?.aborted) throw signal.reason ?? new DOMException('Cancelled', 'AbortError');
    if (!typingIsActive()) return;
  }
}
