import { initialScene, validateScene, type Scene, type Snapshot } from './document';
export interface Repository { load(): Promise<Snapshot>; save(scene: Scene, parent: string | null): Promise<Snapshot> }
interface Revision { id: string; parent: string | null; scene: Scene }
export class BrowserRepository implements Repository {
  private db: Promise<IDBDatabase>;
  constructor(name = 'novel-workspace-spike-v1') {
    this.db = new Promise((resolve, reject) => {
      const request = indexedDB.open(name, 1);
      request.onupgradeneeded = () => { request.result.createObjectStore('revisions', { keyPath: 'id' }); request.result.createObjectStore('metadata'); };
      request.onsuccess = () => { request.result.onversionchange = () => request.result.close(); resolve(request.result); };
      request.onerror = () => reject(new Error('Local browser storage could not open.'));
      request.onblocked = () => reject(new Error('Close other preview tabs, then reopen.'));
    });
  }
  async load(): Promise<Snapshot> {
    const db = await this.db;
    return new Promise((resolve, reject) => {
      const tx = db.transaction(['metadata','revisions'], 'readonly');
      let snapshot: Snapshot = { revisionId: null, scene: structuredClone(initialScene) };
      const head = tx.objectStore('metadata').get('head');
      head.onsuccess = () => {
        if (!head.result) return;
        const revision = tx.objectStore('revisions').get(head.result);
        revision.onsuccess = () => {
          if (!revision.result) { tx.abort(); return; }
          const row = revision.result as Revision;
          snapshot = { revisionId: row.id, scene: row.scene };
        };
      };
      tx.oncomplete = () => { try { validateScene(snapshot.scene); resolve(snapshot); } catch (error) { reject(error); } };
      tx.onabort = () => reject(new Error('The local scene could not be read. Stored data was preserved.'));
      tx.onerror = () => {};
    });
  }
  async save(scene: Scene, parent: string | null): Promise<Snapshot> {
    validateScene(scene);
    const frozen = structuredClone(scene);
    const db = await this.db;
    return new Promise((resolve, reject) => {
      const tx = db.transaction(['metadata','revisions'], 'readwrite', { durability: 'strict' });
      const id = crypto.randomUUID();
      let conflict = false;
      const head = tx.objectStore('metadata').get('head');
      head.onsuccess = () => {
        if ((head.result ?? null) !== parent) { conflict = true; tx.abort(); return; }
        tx.objectStore('revisions').add({ id, parent, scene: frozen } satisfies Revision);
        tx.objectStore('metadata').put(id, 'head');
      };
      tx.oncomplete = () => resolve({ revisionId: id, scene: frozen });
      tx.onabort = () => reject(new Error(conflict ? 'Conflict: another window saved this scene. Export your text before reopening.' : 'Save failed. Your edits are still in this window. Retry or export your text.'));
      tx.onerror = () => {};
    });
  }
}
export function createRepository(): Repository {
  if ('__TAURI_INTERNALS__' in window) {
    return {
      load: async () => { const { invoke } = await import('@tauri-apps/api/core'); const snapshot = await invoke<Snapshot | null>('load_scene'); if (snapshot) validateScene(snapshot.scene); return snapshot ?? { revisionId: null, scene: structuredClone(initialScene) }; },
      save: async (scene, parent) => { validateScene(scene); const { invoke } = await import('@tauri-apps/api/core'); return invoke<Snapshot>('save_scene', { scene, parent }); },
    };
  }
  return new BrowserRepository();
}
