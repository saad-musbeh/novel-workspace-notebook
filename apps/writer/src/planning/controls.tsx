import React, {useState} from 'react';
import { linkLabel, type PlanningDocument, type RecordLink } from './model';
export interface SceneReference { id: string; title: string }
export interface PanelProps { document: PlanningDocument; change: (document: PlanningDocument) => void; scenes: SceneReference[]; onOpenScene?: (id:string) => void }
export function Field({label,children}: {label:string;children:React.ReactNode}) { return <label className="planning-field"><span>{label}</span>{children}</label>; }
export function LinkPicker({document,scenes,value,onChange,optional = true, allowedKinds}: {document:PlanningDocument;scenes:SceneReference[];value:RecordLink|null;onChange:(link:RecordLink|null)=>void;optional?:boolean;allowedKinds?:RecordLink['kind'][]}) {
  const choices: {link:RecordLink;label:string}[] = [...document.records.map(r => ({link:{kind:'record' as const,id:r.id},label:`${r.kind}: ${r.name}`})),...document.plotPoints.map(p=>({link:{kind:'plot-point' as const,id:p.id},label:`Plot point: ${p.title}`})),...document.events.map(e => ({link:{kind:'event' as const,id:e.id},label:`Event: ${e.title}`})),...scenes.map(s => ({link:{kind:'scene' as const,id:s.id},label:`Scene: ${s.title}`}))];
  const current = value ? `${value.kind}:${value.id}` : '';
  return <select aria-label="Linked story record" value={current} onChange={e => onChange(choices.find(c => `${c.link.kind}:${c.link.id}` === e.target.value)?.link ?? null)}><option value="">{optional ? 'No link' : 'Choose a record…'}</option>{value && !choices.some(c => `${c.link.kind}:${c.link.id}` === current) && <option value={current}>{linkLabel(document,scenes,value)}</option>}{choices.filter(c => !allowedKinds || allowedKinds.includes(c.link.kind)).map(c => <option key={`${c.link.kind}:${c.link.id}`} value={`${c.link.kind}:${c.link.id}`}>{c.label}</option>)}</select>;
}
export function LinkedRecords({document,scenes,links,onChange,onOpenScene,allowedKinds}: PanelProps & {links:RecordLink[];onChange:(links:RecordLink[])=>void;allowedKinds?:RecordLink['kind'][]}) {
  return <div className="planning-links">{links.map((link,index) => <div key={`${link.kind}:${link.id}:${index}`}><span>{linkLabel(document,scenes,link)}</span>{link.kind === 'scene' && onOpenScene && <button type="button" onClick={() => onOpenScene(link.id)}>Open scene</button>}<button type="button" aria-label={`Remove link to ${linkLabel(document,scenes,link)}`} onClick={() => onChange(links.filter((_,i) => i !== index))}>×</button></div>)}<LinkPicker document={document} scenes={scenes} value={null} allowedKinds={allowedKinds} onChange={link => { if (link && !links.some(l => l.kind === link.kind && l.id === link.id)) onChange([...links,link]); }}/></div>;
}

export function PagedPlanningList<T>({items,children,empty}:{items:readonly T[];children:(item:T)=>React.ReactNode;empty:string}){
  const [page,setPage]=useState(0);const pages=Math.ceil(items.length/30),current=Math.min(page,Math.max(0,pages-1));
  return <>{!items.length?<p className="planning-muted">{empty}</p>:<ul className="scene-reference-list">{items.slice(current*30,(current+1)*30).map((item,i)=><li key={current*30+i}>{children(item)}</li>)}</ul>}{pages>1&&<div className="planning-toolbar"><button disabled={!current} onClick={()=>setPage(current-1)}>Previous</button><span role="status">{current+1} / {pages} · {items.length} entries</span><button disabled={current+1===pages} onClick={()=>setPage(current+1)}>Next</button></div>}</>;
}
