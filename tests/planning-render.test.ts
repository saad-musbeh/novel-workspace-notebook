import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { StoryBible } from '../apps/writer/src/planning/StoryBible';
import { Board } from '../apps/writer/src/planning/Board';
import { Chronology } from '../apps/writer/src/planning/Chronology';
import { Issues } from '../apps/writer/src/planning/Issues';
import { createPlanningDocument } from '../apps/writer/src/planning/model';
describe('planning UI initial render',()=>{
  it('renders all four planning destinations with an empty local document',()=>{const props={document:createPlanningDocument(),change:()=>{},scenes:[]};for(const Component of [StoryBible,Board,Chronology,Issues])expect(()=>renderToStaticMarkup(React.createElement(Component,props))).not.toThrow();expect(renderToStaticMarkup(React.createElement(Board,props))).toContain('List view');expect(renderToStaticMarkup(React.createElement(Chronology,props))).toContain('Unknown dates remain unresolved');});
  it('shows one shared canonical event in both lanes while rendering manuscript links separately',()=>{const document=createPlanningDocument();document.lanes.push({id:'subplot',name:'Parallel path',kind:'subplot'});document.events.push({id:'shared',title:'نور learns the truth',notes:'',laneIds:document.lanes.map(l=>l.id),sceneIds:['scene-a','scene-b'],time:{kind:'unknown'}});const html=renderToStaticMarkup(React.createElement(Chronology,{document,change:()=>{},scenes:[{id:'scene-a',title:'Flashback'},{id:'scene-b',title:'Present'}]}));expect(html).toContain('نور learns the truth');expect(html).toContain('2 scene links');expect(document.events).toHaveLength(1);});
  it('omits hidden custom fields while leaving their stored multilingual values intact',()=>{const document=createPlanningDocument();document.fields=[{id:'hidden',label:'Secret field',kind:'text',recordKind:'character',hidden:true,options:[]}];document.records=[{id:'person',kind:'character',name:'明',aliases:[],notes:'',links:[],values:{hidden:'مَرْحَبًا'}}];const html=renderToStaticMarkup(React.createElement(StoryBible,{document,change:()=>{},scenes:[]}));expect(html).not.toContain('Secret field');expect(document.records[0].values.hidden).toBe('مَرْحَبًا');});
});
