import {describe,expect,it} from 'vitest';
import {defaultDeskSettings,parseDeskSettings} from '../apps/writer/src/desk-settings';
import {defaultShortcutSettings,parseShortcutSettings} from '../apps/writer/src/shortcuts';
describe('writing desk preferences',()=>{
 it('keeps shell direction independent from Unicode manuscript content',()=>{expect(parseDeskSettings({...defaultDeskSettings(),direction:'rtl',writingWidth:800})).toEqual({...defaultDeskSettings(),direction:'rtl',writingWidth:800});});
 it('bounds widths and refuses unknown fields or invalid booleans',()=>{for(const invalid of [null,{}, {...defaultDeskSettings(),outline:'yes'},{...defaultDeskSettings(),writingWidth:1201},{...defaultDeskSettings(),writingWidth:359},{...defaultDeskSettings(),writingWidth:NaN},{...defaultDeskSettings(),extra:true}])expect(()=>parseDeskSettings(invalid)).toThrow();expect(parseDeskSettings(defaultDeskSettings()).writingWidth).toBe(0);});
 it('migrates peek and navigation shortcuts without replacing user bindings',()=>{const previous=defaultShortcutSettings() as any;for(const id of ['workspace.peek','workspace.navigateBack','workspace.navigateForward'])delete previous.bindings[id];previous.bindings['workspace.save']=[{code:'F6',ctrl:false,alt:false,shift:false,meta:false}];const restored=parseShortcutSettings(previous);expect(restored.bindings['workspace.peek']).toEqual([]);expect(restored.bindings['workspace.save']).toEqual(previous.bindings['workspace.save']);});
});
