import { afterEach, describe, expect, it, vi } from 'vitest';
import { Autosave } from '../apps/writer/src/autosave';
import { initialScene } from '../apps/writer/src/document';
afterEach(() => vi.useRealTimers());
describe('autosave acknowledgement and ordering', () => {
  it('waits for composition to end and the transaction to complete', async () => {
    vi.useFakeTimers();
    let finish!: (value: any) => void;
    const save = vi.fn(() => new Promise<any>(resolve => { finish = resolve; }));
    const status = vi.fn(); const saver = new Autosave({ load: vi.fn(), save }, null, status);
    saver.composition(true); saver.change(initialScene);
    await vi.advanceTimersByTimeAsync(2000); expect(save).not.toHaveBeenCalled();
    saver.composition(false); await vi.advanceTimersByTimeAsync(500);
    expect(status).toHaveBeenLastCalledWith('saving');
    finish({ revisionId: 'r1', scene: initialScene }); await vi.advanceTimersByTimeAsync(0);
    expect(status).toHaveBeenLastCalledWith('saved'); saver.dispose();
  });
  it('serializes edits made during a save against the acknowledged parent', async () => {
    vi.useFakeTimers(); let finish!: (value: any) => void;
    const save = vi.fn().mockImplementationOnce(() => new Promise(resolve => { finish = resolve; })).mockResolvedValue({ revisionId: 'r2' });
    const saver = new Autosave({ load: vi.fn(), save }, null, vi.fn());
    saver.change(initialScene); await vi.advanceTimersByTimeAsync(500);
    saver.change({ ...initialScene, title: 'Newest' }); await vi.advanceTimersByTimeAsync(500); expect(save).toHaveBeenCalledTimes(1);
    finish({ revisionId: 'r1' }); await vi.advanceTimersByTimeAsync(1000);
    expect(save).toHaveBeenLastCalledWith(expect.objectContaining({ title: 'Newest' }), 'r1'); saver.dispose();
  });
  it('keeps the newest unsaved text after failure and retries only explicitly', async () => {
    vi.useFakeTimers(); const status = vi.fn(); const save = vi.fn().mockRejectedValueOnce(new Error('Storage full')).mockResolvedValue({ revisionId: 'r1' });
    const saver = new Autosave({ load: vi.fn(), save }, null, status);
    saver.change(initialScene); await vi.advanceTimersByTimeAsync(500);
    expect(status).toHaveBeenLastCalledWith('error', 'Storage full');
    saver.change({ ...initialScene, title: 'Keep this' }); await vi.advanceTimersByTimeAsync(5000);
    expect(save).toHaveBeenCalledTimes(1); saver.retry(); await vi.advanceTimersByTimeAsync(0);
    expect(save).toHaveBeenLastCalledWith(expect.objectContaining({ title: 'Keep this' }), null);
    expect(status).toHaveBeenLastCalledWith('saved'); saver.dispose();
  });
  it('drains an in-flight commit and the newer edit before allowing close', async () => {
    let finish!: (value: any) => void;
    const save = vi.fn().mockImplementationOnce(() => new Promise(resolve => { finish = resolve; })).mockResolvedValue({ revisionId: 'r2' });
    const saver = new Autosave({ load: vi.fn(), save }, null, vi.fn());
    saver.change(initialScene); const first = saver.flush(); await Promise.resolve();
    saver.change({ ...initialScene, title: 'مِ中文 newest' });
    let closed = false; const close = saver.drain().then(() => { closed = true; });
    await Promise.resolve(); expect(closed).toBe(false);
    finish({ revisionId: 'r1' }); await first; await close;
    expect(save).toHaveBeenLastCalledWith(expect.objectContaining({ title: 'مِ中文 newest' }), 'r1');
    expect(closed).toBe(true); saver.dispose();
  });
  it('refuses close on storage failure, preserving text for an explicit retry', async () => {
    const save = vi.fn().mockRejectedValueOnce(new Error('Storage full')).mockResolvedValue({ revisionId: 'r1' });
    const saver = new Autosave({ load: vi.fn(), save }, null, vi.fn());
    saver.change(initialScene); await expect(saver.drain()).rejects.toThrow('Storage full');
    await expect(saver.drain()).rejects.toThrow('Storage full'); expect(save).toHaveBeenCalledTimes(1);
    saver.retry(); await saver.drain(); expect(save).toHaveBeenCalledTimes(2); saver.dispose();
  });
  it('refuses close during composition and saves after composition finishes', async () => {
    const save = vi.fn().mockResolvedValue({ revisionId: 'r1' });
    const saver = new Autosave({ load: vi.fn(), save }, null, vi.fn());
    saver.composition(true); saver.change(initialScene);
    await expect(saver.drain()).rejects.toThrow('Finish entering'); expect(save).not.toHaveBeenCalled();
    saver.composition(false); await saver.drain(); expect(save).toHaveBeenCalledTimes(1); saver.dispose();
  });
});
