import { validatePlanningDocument, type PlanningAdapter, type PlanningDocument } from './model';
export type PlanningSaveStatus = 'unsaved' | 'saving' | 'saved' | 'error';
/** Serializes writes; stale parents fail without losing the local edit or overwriting canon. */
export class PlanningSaver {
  private pending: PlanningDocument | null = null;
  private active: Promise<void> | null = null;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private failure = '';
  private disposed = false;
  private composing = false;
  constructor(private adapter: PlanningAdapter, private projectId: string, private parent: string | null, private notify: (status: PlanningSaveStatus, error?: string) => void, private delay = 450) {}
  change(document: PlanningDocument) { validatePlanningDocument(document); this.pending = structuredClone(document); this.notify(this.failure ? 'error' : 'unsaved',this.failure); this.schedule(); }
  composition(active: boolean) { this.composing = active; clearTimeout(this.timer); if (!active) this.schedule(); }
  private schedule() { clearTimeout(this.timer); if (!this.failure && !this.disposed && !this.composing) this.timer = setTimeout(() => void this.flush().catch(() => {}),this.delay); }
  async flush(): Promise<void> {
    clearTimeout(this.timer); if (this.active) { await this.active; return this.flush(); } if (this.failure) throw new Error(this.failure); if (!this.pending) return;
    if (this.composing) throw new Error('Finish entering the current word before saving planning changes.');
    const document = this.pending; this.pending = null; this.notify('saving');
    this.active = Promise.resolve().then(async () => {
      try { const saved = await this.adapter.save(this.projectId,document,this.parent); validatePlanningDocument(saved.document); this.parent = saved.revisionId; this.notify(this.pending ? 'unsaved' : 'saved'); }
      catch (error) { this.pending ??= document; this.failure = String(error); this.notify('error',this.failure); throw error; }
    });
    try { await this.active; } finally { this.active = null; if (this.pending) this.schedule(); }
  }
  async drain() { while (this.active || this.pending) await this.flush(); if (this.failure) throw new Error(this.failure); }
  retry() { this.failure = ''; return this.drain(); }
  dispose() { this.disposed = true; clearTimeout(this.timer); }
}
