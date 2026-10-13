import {expect,it} from 'vitest';
import {validateScene,type Scene} from '../apps/writer/src/document';

function sample(lineSpacing:unknown,nodeType='paragraph'):Scene {
  return {schemaVersion:1,title:'Spacing',document:{type:'doc',content:[{type:nodeType,attrs:{dir:'rtl',lang:'ar',lineSpacing},content:[{type:'text',text:'يس مِ English 中文'}]}]}};
}
it('accepts supported paragraph and heading spacing without changing text or legacy defaults',()=>{
  for(const value of [1,1.15,1.5,2])for(const kind of ['paragraph','heading']){
    const scene=sample(value,kind),before=JSON.stringify(scene);validateScene(scene);expect(JSON.stringify(scene)).toBe(before);
  }
  const legacy=sample(undefined);validateScene(legacy);expect(legacy.document.content?.[0].attrs?.lineSpacing).toBeUndefined();
});
it('refuses unsupported spacing values and use on inline nodes',()=>{
  for(const value of [0,1.1,3,'1',null,true,NaN,Infinity])expect(()=>validateScene(sample(value))).toThrow('Unsupported line spacing');
  const inline:Scene={schemaVersion:1,title:'Inline',document:{type:'doc',content:[{type:'paragraph',content:[{type:'text',text:'يس',attrs:{lineSpacing:1}}]}]}};
  expect(()=>validateScene(inline)).toThrow('Unsupported line spacing');
});
