import 'fake-indexeddb/auto';
import { describe, expect, it } from 'vitest';
import { BrowserRepository } from '../apps/writer/src/storage';
import { initialScene, graphemeCount, plainText, validateScene } from '../apps/writer/src/document';
const repository = (name = crypto.randomUUID()) => new BrowserRepository(name);
describe('local scene persistence', () => {
  it('round-trips mixed scripts, formatting, private-use and supplementary code points', async () => {
    const name = crypto.randomUUID(); const db = repository(name);
    const scene = structuredClone(initialScene);
    scene.document.content!.push({ type: 'paragraph', attrs: { dir: 'rtl', lang: 'und' }, content: [{ type: 'text', text: '\uE000 𠀀 👩🏽‍🚀 مَرْحَبًا', marks: [{ type: 'bold' }] }] });
    const saved = await db.save(scene, null);
    expect(await repository(name).load()).toEqual(saved);
    expect(await db.save({ ...scene, title: 'Renamed' }, saved.revisionId)).toMatchObject({ scene: { title: 'Renamed' } });
  });
  it('rejects an outdated window without replacing the committed scene', async () => {
    const name = crypto.randomUUID(); const first = repository(name); const second = repository(name);
    const saved = await first.save(initialScene, null);
    await expect(second.save({ ...initialScene, title: 'stale' }, null)).rejects.toThrow('Conflict');
    expect(await second.load()).toEqual(saved);
  });
  it('rejects unsupported versions before a write', async () => {
    const db = repository();
    await expect(db.save({ ...initialScene, schemaVersion: 2 } as never, null)).rejects.toThrow('Unsupported');
    expect((await db.load()).revisionId).toBeNull();
  });
  it('does not count combining marks or emoji components as separate graphemes', () => {
    expect(graphemeCount('a\u0301👩🏽‍🚀中')).toBe(3);
    expect(plainText(initialScene.document)).toContain('故事從這裡開始。');
  });
  it('rejects remote image nodes and unsupported formatting', () => {
    expect(() => validateScene({ ...initialScene, document: { type: 'doc', content: [{ type: 'image', attrs: { src: 'https://example.com/image' } }] } })).toThrow('Unsupported');
  });
});
