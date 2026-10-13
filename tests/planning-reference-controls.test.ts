import {beforeEach,describe,expect,it,vi} from 'vitest';
import {createPlanningDocument,type PlanningDocument} from '../apps/writer/src/planning/model';
import type {SceneReferencePaneProps} from '../apps/writer/src/planning/SceneReferencePane';
const hooks=vi.hoisted(()=>({slots:[] as any[],cursor:0}));
vi.mock('react',async()=>{
 const actual=await vi.importActual<any>('react');
 return {...actual,useState:(initial:any)=>{const i=hooks.cursor++,slot=hooks.slots[i]??={value:typeof initial==='function'?initial():initial};return [slot.value,(value:any)=>{slot.value=typeof value==='function'?value(slot.value):value;}];},useMemo:(factory:()=>any,deps:any[])=>{const i=hooks.cursor++,prior=hooks.slots[i];if(!prior||prior.deps.length!==deps.length||deps.some((value,index)=>!Object.is(value,prior.deps[index])))hooks.slots[i]={value:factory(),deps};return hooks.slots[i].value;}};
});
import {SceneReferencePane} from '../apps/writer/src/planning/SceneReferencePane';
import {Chronology} from '../apps/writer/src/planning/Chronology';
import {CharacterKnowledge} from '../apps/writer/src/planning/CharacterKnowledge';
function nodes(node:any):any[]{if(Array.isArray(node))return node.flatMap(nodes);return node&&typeof node==='object'?[node,...nodes(node.props?.children)]:[];}
function text(node:any):string {return Array.isArray(node)?node.map(text).join(''):node&&typeof node==='object'?text(node.props?.children):node==null?'':String(node);}
const scenes=[{id:'first',title:'First مِ'},{id:'second',title:'Second 中文'}];
function fixture():PlanningDocument {const doc=createPlanningDocument();doc.records=[{id:'hero',kind:'character',name:'نور 明',aliases:[],notes:'',values:{},links:[{kind:'scene',id:'first'}]}];doc.facts=[{id:'fact',subjectId:'hero',property:'name',value:'نور 明',status:'fact',confirmed:true,observerId:null,source:'Author',eventId:null,untilEventId:null}];doc.knowledge=[{id:'known',characterId:'hero',factId:'fact',stance:'knows',factSnapshot:{subjectId:'hero',property:'name',value:'نور 明'},interpretation:'',source:'Author scene scope',scope:{kind:'scenes',sceneIds:['first']}}];return doc;}
let props:SceneReferencePaneProps,tree:any;
function render(){hooks.cursor=0;tree=SceneReferencePane(props);return tree;}
function button(label:string){return nodes(tree).find(node=>node.type==='button'&&text(node)===label);}
function sceneSelect(){return nodes(tree).find(node=>node.props?.['aria-label']==='Reference scene');}
beforeEach(()=>{hooks.slots=[];hooks.cursor=0;props={document:fixture(),scenes,sceneId:'first',onOpenScene:vi.fn(),onClose:vi.fn()};});
describe('reference pane controls',()=>{
 it('follows current scene visibly until pinned, then retains the pinned scene without changing source data',()=>{
  const frozen=JSON.stringify(props.document);render();expect(sceneSelect().props.value).toBe('');expect(text(tree)).toContain('First مِ');button('Pin scene').props.onClick();render();expect(sceneSelect().props.value).toBe('first');
  props={...props,sceneId:'second'};render();expect(text(tree)).toContain('First مِ');button('Open scene').props.onClick();expect(props.onOpenScene).toHaveBeenCalledExactlyOnceWith('first');
  button('Follow current scene').props.onClick();render();expect(sceneSelect().props.value).toBe('');expect(nodes(tree).find(node=>node.props?.className==='scene-reference-title').props.children).toBe('Second 中文');expect(JSON.stringify(props.document)).toBe(frozen);
 });
 it('pins the chosen scene and keeps a missing pin explicit rather than falling back to current scene',()=>{
  render();sceneSelect().props.onChange({target:{value:'second'}});render();expect(sceneSelect().props.value).toBe('second');props={...props,scenes:[scenes[0]],sceneId:'first'};render();expect(text(tree)).toContain('This scene is no longer in the manuscript.');expect(button('Open scene')).toBeUndefined();expect(sceneSelect().props.value).toBe('second');button('Follow current scene').props.onClick();render();expect(nodes(tree).find(node=>node.props?.className==='scene-reference-title').props.children).toBe('First مِ');
 });
 it('does no projection when inactive and retains pin on reactivation',()=>{
  render();button('Pin scene').props.onClick();render();const original=props.document;props={...props,active:false,document:new Proxy(original!,{get(){throw new Error('Inactive pane read metadata');}})};expect(render()).toBeNull();props={...props,active:true,document:original,sceneId:'second'};render();expect(sceneSelect().props.value).toBe('first');
 });
 it('does not rebuild the metadata index for scene-only changes and exposes read-only knowledge',()=>{
  const original=props.document!,records=original.records;let reads=0;Object.defineProperty(original,'records',{get:()=>{reads++;return records;}});render();const initialReads=reads;props={...props,sceneId:'second'};render();expect(reads).toBe(initialReads);props={...props,sceneId:'first'};render();button('Knowledge').props.onClick();render();const list=nodes(tree).find(node=>typeof node.type!=='string'&&node.type?.name==='PagedPlanningList');expect(list.props.items[0]).toMatchObject({applicability:'applies',entry:{stance:'knows',factSnapshot:{value:'نور 明'}}});expect(nodes(tree).some(node=>node.type==='textarea')).toBe(false);button('Close').props.onClick();expect(props.onClose).toHaveBeenCalledTimes(1);
 });
 it('offers a clear empty state without creating a planning document or facts',()=>{
  props={...props,document:null};render();expect(text(tree)).toContain('No linked story records/events yet.');expect(props.document).toBeNull();
 });
});
describe('authored timeline and knowledge navigation',()=>{
 it('switching to reader order does not turn occurrence links into reveals or edit planning',()=>{
  const document=fixture(),frozen=JSON.stringify(document),change=vi.fn(),panelProps={document,scenes,change};hooks.cursor=0;let output=Chronology(panelProps);nodes(output).find(node=>node.type==='button'&&text(node)==='Reader learns').props.onClick();hooks.cursor=0;output=Chronology(panelProps);expect(nodes(output).some(node=>node.type?.name==='ReaderTimeline')).toBe(true);expect(change).not.toHaveBeenCalled();expect(JSON.stringify(document)).toBe(frozen);expect(document.readerReveals).toBeUndefined();
 });
 it('scene selection shows author-grounded scope without creating knowledge or changing canon',()=>{
  const document=fixture(),change=vi.fn(),panelProps={document,scenes,change,characterId:'hero'},frozen=JSON.stringify(document);hooks.cursor=0;let output=CharacterKnowledge(panelProps);let list=nodes(output).find(node=>node.type?.name==='PagedPlanningList');expect(list.props.items[0].applicability).toBe('applies');nodes(output).find(node=>node.type==='select').props.onChange({target:{value:'second'}});hooks.cursor=0;output=CharacterKnowledge(panelProps);list=nodes(output).find(node=>node.type?.name==='PagedPlanningList');expect(list.props.items[0].applicability).toBe('outside-scenes');expect(change).not.toHaveBeenCalled();expect(JSON.stringify(document)).toBe(frozen);
 });
});
