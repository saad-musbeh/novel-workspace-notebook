import type { Scene } from './document';
import type { Repository } from './storage';
export type SaveStatus = 'unsaved' | 'saving' | 'saved' | 'error';
export class Autosave {
  private pending: Scene | null = null;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private active: Promise<void> | null = null;
  private composing = false;
  private failed = false;
  private failureMessage = '';
  constructor(private repository: Repository, private parent: string | null,
    private onStatus: (status: SaveStatus, error?: string) => void, private delay = 450) {}
  change(scene: Scene): void { this.pending = structuredClone(scene); if (!this.failed) this.onStatus('unsaved'); this.schedule(); }
  composition(active: boolean): void { this.composing = active; clearTimeout(this.timer); if (!active) this.schedule(); }
  private schedule(): void { clearTimeout(this.timer); if (!this.composing && !this.failed) this.timer = setTimeout(() => void this.flush(), this.delay); }
  async flush(): Promise<void> {
    clearTimeout(this.timer);
    if (this.active) { await this.active; return this.flush(); }
    if (this.composing || !this.pending || this.failed) return;
    this.active = Promise.resolve().then(() => this.commit());
    try { await this.active; }
    finally { this.active = null; if (this.pending && !this.failed) this.schedule(); }
  }
  private async commit(): Promise<void> {
    if (!this.pending) return;
    const scene = this.pending; this.pending = null;
    this.onStatus('saving');
    try {
      const saved = await this.repository.save(scene, this.parent);
      this.parent = saved.revisionId;
      this.onStatus(this.pending ? 'unsaved' : 'saved');
    } catch (error) {
      this.pending ??= scene; this.failed = true;
      this.failureMessage = error instanceof Error ? error.message : String(error);
      this.onStatus('error', this.failureMessage);
    }
  }
  /** Close only after every confirmed edit has actually committed. */
  async drain(): Promise<void> {
    clearTimeout(this.timer);
    while (this.active || this.pending) {
      if (this.composing) throw new Error('Finish entering the current word, then close again.');
      if (this.failed) throw new Error(this.failureMessage);
      await this.flush();
    }
    if (this.composing) throw new Error('Finish entering the current word, then close again.');
    if (this.failed) throw new Error(this.failureMessage);
  }
  retry(): void { this.failed = false; void this.flush(); }
  dispose(): void { clearTimeout(this.timer); }
}
