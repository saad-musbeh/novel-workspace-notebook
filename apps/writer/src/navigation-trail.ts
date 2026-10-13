import type {TabRoute} from './workspace-tabs';
export interface NativeCaretBookmark {documentHash:string;cursor:[number,number];anchor:[number,number]}
export interface NavigationBookmark {native?:NativeCaretBookmark;reading:{scrollTop:number;bookSceneId?:string;bookSceneOffset?:number;bookCharacterId?:string}}
export interface NavigationController {capture():Promise<NavigationBookmark>;restore(bookmark:NavigationBookmark,signal?:AbortSignal):Promise<void>}
export interface TrailLocation {tabId:string;route:TabRoute;bookmark?:NavigationBookmark}
export interface NavigationTicket {readonly epoch:number;readonly index:number;readonly target:TrailLocation}
const same=(a:TrailLocation,b:TrailLocation)=>a.tabId===b.tabId&&a.route.projectId===b.route.projectId&&a.route.view===b.route.view&&(a.route.view!=='write'||a.route.sceneId===b.route.sceneId);
const copy=(location:TrailLocation):TrailLocation=>structuredClone(location);
/** Commit after ownership, save and restoration checks have succeeded. */
export class NavigationTrail {
 private entries:TrailLocation[]=[];private index=-1;private epoch=0;
 constructor(private readonly limit=100){if(!Number.isInteger(limit)||limit<2||limit>1000)throw new Error('Invalid navigation trail limit.');}
 get current(){return this.index<0?null:copy(this.entries[this.index]);}
 get canBack(){return this.index>0;}
 get canForward(){return this.index>=0&&this.index<this.entries.length-1;}
 updateCurrent(location:TrailLocation){if(this.index<0||!same(this.entries[this.index],location))return false;this.entries[this.index]=copy(location);this.epoch++;return true;}
 visit(destination:TrailLocation,departure?:TrailLocation){
  if(departure)this.updateCurrent(departure);
  if(this.index>=0&&same(this.entries[this.index],destination)){this.entries[this.index]=copy({...destination,bookmark:destination.bookmark??this.entries[this.index].bookmark});this.epoch++;return;}
  this.entries=this.entries.slice(0,this.index+1);this.entries.push(copy(destination));
  if(this.entries.length>this.limit)this.entries.shift();this.index=this.entries.length-1;this.epoch++;
 }
 prune(isAvailable:(location:TrailLocation)=>boolean){
  const kept=this.entries.map((location,index)=>({location,index})).filter(entry=>isAvailable(copy(entry.location)));
  if(kept.length===this.entries.length)return;
  const exact=kept.findIndex(entry=>entry.index===this.index);
  const predecessor=kept.reduce((last,entry,index)=>entry.index<this.index?index:last,-1);
  this.entries=kept.map(entry=>entry.location);
  this.index=exact>=0?exact:predecessor>=0?predecessor:kept.length?0:-1;this.epoch++;
 }
 removeTab(tabId:string){this.prune(location=>location.tabId!==tabId);}
 begin(direction:'back'|'forward'):NavigationTicket|null{const index=this.index+(direction==='back'?-1:1);return index<0||index>=this.entries.length?null:{epoch:this.epoch,index,target:copy(this.entries[index])};}
 commit(ticket:NavigationTicket){if(ticket.epoch!==this.epoch||ticket.index<0||ticket.index>=this.entries.length||!same(ticket.target,this.entries[ticket.index]))return false;this.index=ticket.index;this.epoch++;return true;}
}
