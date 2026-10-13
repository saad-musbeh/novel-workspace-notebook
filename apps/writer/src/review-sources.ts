import {plainText, type DocumentNode} from './document';
import {sha256, type SceneSnapshot} from './assistance/model';
import type {SceneRecord} from './workspace';
import {typingIsActive,waitForTypingIdle} from './typing-priority';

interface Entry {revisionId:string; document:DocumentNode; text:string; hash:Promise<string>}
// Retain only the open project's sources. A save need not rehash other scenes.
export class ReviewSources {
  private entries=new Map<string,Entry>();
  private generation=0;
  constructor(private digest:(text:string)=>Promise<string>=sha256) {}
  async refresh(scenes:readonly SceneRecord[],signal?:AbortSignal):Promise<SceneSnapshot[]> {
    signal?.throwIfAborted();
    const generation=++this.generation,previousEntries=this.entries;
    const next=new Map<string,Entry>();
    const sources:Promise<SceneSnapshot>[]=[];
    let turn=performance.now();
    try {for(const record of scenes){
      if(typingIsActive())await waitForTypingIdle(signal);
      else if(typeof window!=='undefined'&&performance.now()-turn>=4){await new Promise(resolve=>setTimeout(resolve,0));turn=performance.now();}
      signal?.throwIfAborted();
      const previous=previousEntries.get(record.id);
      const document=record.scene.document;
      const text=previous?.document===document&&previous.revisionId===record.revisionId?previous.text:plainText(document);
      const unchanged=previous?.revisionId===record.revisionId&&previous.text===text;
      const entry:Entry={revisionId:record.revisionId,document,text,hash:unchanged?previous.hash:this.digest(text)};
      next.set(record.id,entry);
      const snapshot={sceneId:record.id,revisionId:record.revisionId,text,title:record.scene.title,language:String(document.content?.[0]?.attrs?.lang??'en')};
      const source=entry.hash.then(hash=>({...snapshot,hash}));
      // A later yield/abort must not leave a started digest rejection unhandled.
      void source.catch(()=>undefined);sources.push(source);
    }
    if(generation===this.generation)this.entries=next;
    const values=await Promise.all(sources);signal?.throwIfAborted();return values;}
    catch(error) {
      // Retry failed digests; never let an older request clear newer entries.
      for(const [id,entry] of next)if(this.entries.get(id)===entry)this.entries.delete(id);
      throw error;
    }
  }
}
