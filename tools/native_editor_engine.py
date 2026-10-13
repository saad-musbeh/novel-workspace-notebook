"""Private stdio manuscript editor: our model and Pango/Cairo, no office process."""
import ctypes as C
import ctypes.util
import hashlib,json,math,os,sys,time,traceback
from pathlib import Path
from native_text_model import TextModel,validate_document,validate_text
from native_text_layout import NativeLayout
from native_editor_checkpoint import write_checkpoint,read_checkpoint

def register_project_fonts(directory,fonts):
    """Register only checked project fonts in this disposable editor process."""
    if not isinstance(fonts,list) or len(fonts)>8:raise ValueError('Invalid project font list.')
    if not fonts:return []
    root=(Path(directory)/'fonts').resolve();validated=[]
    for font in fonts:
        if not isinstance(font,dict) or not isinstance(font.get('path'),str):raise ValueError('Invalid project font asset.')
        path=Path(font['path']).resolve()
        if not path.is_relative_to(root) or not path.is_file() or path.stat().st_size>16*1024*1024:raise ValueError('Project font is outside the private editor session or exceeds its size limit.')
        content=path.read_bytes()
        if len(content)<12 or content[:4] not in [b'\x00\x01\x00\x00',b'OTTO',b'true']:raise ValueError('Project font must be a single-font TTF or OTF.')
        if hashlib.sha256(content).hexdigest()!=font.get('checksum'):raise ValueError('Project font checksum failed. Original manuscript text is preserved.')
        family=font.get('family')
        if not isinstance(family,str) or not family.strip() or len(family)>200 or any(ord(char)<32 for char in family):raise ValueError('Invalid project font family.')
        validated.append((path,family))
    library=C.CDLL(C.util.find_library('fontconfig') or 'libfontconfig.so.1')
    library.FcConfigGetCurrent.restype=C.c_void_p
    library.FcConfigAppFontAddFile.argtypes=[C.c_void_p,C.c_char_p];library.FcConfigAppFontAddFile.restype=C.c_int
    library.FcFreeTypeQuery.argtypes=[C.c_char_p,C.c_uint,C.c_void_p,C.POINTER(C.c_int)];library.FcFreeTypeQuery.restype=C.c_void_p
    library.FcPatternGetString.argtypes=[C.c_void_p,C.c_char_p,C.c_int,C.POINTER(C.c_char_p)];library.FcPatternGetString.restype=C.c_int
    library.FcPatternDestroy.argtypes=[C.c_void_p]
    configuration=library.FcConfigGetCurrent()
    if not configuration:raise RuntimeError('The installed font engine is unavailable.')
    for path,family in validated:
        count=C.c_int();pattern=library.FcFreeTypeQuery(os.fsencode(path),0,None,C.byref(count))
        if not pattern:raise ValueError('The imported project font could not be read by Fontconfig.')
        try:
            names=[];index=0;name=C.c_char_p()
            while library.FcPatternGetString(pattern,b'family',index,C.byref(name))==0:
                names.append(name.value.decode('utf-8'));index+=1
            if family not in names:raise ValueError('The imported project font family does not match its definition.')
        finally:library.FcPatternDestroy(pattern)
        if not library.FcConfigAppFontAddFile(configuration,os.fsencode(path)):raise RuntimeError('The imported project font could not be loaded in this editor process.')
    return [family for _,family in validated]

def writing_system_font(selection,families):
    if selection is None:return None
    if not isinstance(selection,dict) or not isinstance(selection.get('definitionId'),str) or not selection['definitionId'] or type(selection.get('mappingVersion')) is not int or selection['mappingVersion']<1 or selection.get('direction') not in ('auto','ltr','rtl'):raise ValueError('Invalid selected alphabet definition or mapping version.')
    family=selection.get('fontFamily')
    if family is not None and family not in families:raise ValueError('The selected alphabet font is unavailable in this project. Original text is preserved.')
    return family

class NativeEditor:
    def __init__(self,directory,request):
        self.directory=Path(directory);self.opening=request
        scene=request.get('scene')
        if not isinstance(scene,dict) or type(scene.get('schemaVersion')) is not int or scene['schemaVersion']!=1 or not isinstance(scene.get('title'),str) or len(scene['title'])>500:raise ValueError('Invalid manuscript scene.')
        serialized=lambda value:len(json.dumps(value,ensure_ascii=False,separators=(',',':')).encode('utf-8'))
        if serialized(scene)>2_000_000:raise ValueError('This editor supports scenes up to 2 MB.')
        width=request.get('width',640)
        if type(width) is not int or not 240<=width<=4096:raise ValueError('Invalid editor width.')
        validate_document(scene.get('document'))
        families=register_project_fonts(directory,request.get('fonts',[]))
        family=writing_system_font(request.get('writingSystem'),families)
        self.model=TextModel(scene['document']);self.layout=NativeLayout(self.model,width,family)
        self.model.max_document_bytes=2_000_000-(serialized(scene)-serialized(scene['document']))
        self.top=0;self.height=680;self.search_result=None;self.unicode_result=None;self.mouse_count=1
    def paragraph_capture(self,index=None):
        index=self.model.cursor[0] if index is None else index
        if type(index) is not int or not 0<=index<len(self.model.blocks):raise ValueError('This paragraph is no longer in the scene.')
        document=self.model.document_snapshot()
        digest=hashlib.sha256(json.dumps(document,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode('utf-8')).hexdigest()
        return {'blockIndex':index,'documentHash':digest,'block':json.loads(json.dumps(self.model.blocks[index],ensure_ascii=False))}

    def navigation_capture(self):
        digest=hashlib.sha256(json.dumps(self.model.document_snapshot(),ensure_ascii=False,sort_keys=True,separators=(',',':')).encode('utf-8')).hexdigest()
        return {'documentHash':digest,'cursor':list(self.model.cursor),'anchor':list(self.model.anchor)}

    def _viewport(self,request):
        top=request.get('top',self.top);height=request.get('viewportHeight',self.height);follow=request.get('followCursor',False);preview=request.get('preview',False)
        if type(height) is not int or not 120<=height<=2160:raise ValueError('Invalid viewport height.')
        if type(top) is not int or not 0<=top<=max(1_000_000,math.ceil(self.layout.total_height+height)):raise ValueError('Invalid viewport.')
        if type(follow) is not bool or type(preview) is not bool:raise ValueError('Invalid editor display option.')
        return top,height,follow,preview
    def request(self,request,opening=False):
        # A shaping/painting failure is an unacknowledged operation. Restore
        # document, selection and history together; snapshots share immutable
        # blocks and copy only short history/reference lists, never draft text.
        before=self.model._state();undo=self.model.undo[:];redo=self.model.redo[:]
        bookkeeping=(self.model.undo_bytes,self.model.undo_trimmed,self.model.version,self.model._unicode_result)
        viewport=(self.top,self.height,self.layout.width)
        try:return self._request(request,opening)
        except Exception:
            self.model._restore(before);self.model.undo=undo;self.model.redo=redo
            self.model.undo_bytes,self.model.undo_trimmed,self.model.version,self.model._unicode_result=bookkeeping
            self.model.prune_caches()
            self.top,self.height,width=viewport
            self.layout.set_width(width);self.layout.reset_navigation()
            self.search_result=None;self.unicode_result=None
            raise
    def _request(self,request,opening=False):
        started=time.perf_counter()
        if not isinstance(request,dict):raise ValueError('Invalid editor operation.')
        top,height,follow,preview=self._viewport(request)
        kind=request.get('type');self.search_result=None;self.unicode_result=None;capture=None;navigation=None
        if kind not in ('open','text','key','mouse','command','refresh','snapshot'):raise ValueError('Unsupported editor operation.')
        width=request.get('width')
        if width is not None and (type(width) is not int or not 240<=width<=4096):raise ValueError('Invalid editor width.')
        if kind=='open' and not opening:raise ValueError('Open a new private editor instance for this scene.')
        # Validate every pointer/key parameter before selection or document work.
        if kind=='mouse':
            x=request.get('x');y=request.get('y');count=request.get('count',1);phase=request.get('phase')
            if type(x) is not int or type(y) is not int or not 0<=x<=1_000_000 or not 0<=y<=max(1_000_000,math.ceil(self.layout.total_height+height)*15) or type(count) is not int or count not in (1,2,3) or phase not in ('down','move','up'):raise ValueError('Invalid manuscript pointer operation.')
            if 'buttons' in request and type(request['buttons']) is not bool:raise ValueError('Invalid pointer buttons.')
        if kind=='key' and (request.get('key') not in ('ArrowUp','ArrowDown','ArrowLeft','ArrowRight','Home','End','PageUp','PageDown','Enter','Backspace','Delete','Tab','Escape') or any(key in request and type(request[key]) is not bool for key in ('shift','ctrl'))):raise ValueError('Unsupported manuscript key.')
        if kind=='mouse' and 'shift' in request and type(request['shift']) is not bool:raise ValueError('Invalid pointer modifier.')
        inputs=request.get('inputs')
        if inputs is not None:
            if kind!='text' or not isinstance(inputs,list) or not 1<=len(inputs)<=128 or any(not isinstance(text,str) for text in inputs):raise ValueError('Invalid ordered typing batch.')
            if sum(len(text.encode('utf-8')) for text in inputs)>256*1024 or ''.join(inputs)!=request.get('text'):raise ValueError('Typing batch exceeds its limit or does not match the original input.')
            for text in inputs:validate_text(text)
        defer=request.get('deferRender',False)
        if type(defer) is not bool or (defer and kind!='text'):raise ValueError('Only ordinary text can defer rendering.')
        if width is not None and width!=self.layout.width:
            if follow:self.layout.set_width(width)
            else:top=self.layout.reflow(top,width)
        self.top=top;self.height=height;changed=False
        if kind=='mouse':
            point=self.layout.locate_point(x/15,y/15)
            if phase=='down':
                self.layout.reset_navigation();self.mouse_count=count
                if request.get('shift'):self.model.move_to(point,True)
                elif count==2:self.model.select_word(point)
                elif count==3:self.model.select_paragraph(point)
                else:self.model.move_to(point)
            elif phase=='move' and request.get('buttons'):self.model.move_to(point,True)
            elif phase=='up' and self.mouse_count==1:self.model.move_to(point,True)
        elif kind=='key':
            key=request['key'];shift=request.get('shift',False);ctrl=request.get('ctrl',False)
            if key in ('ArrowUp','ArrowDown','PageUp','PageDown'):
                delta=1 if key in ('ArrowDown','PageDown') else -1
                if key.startswith('Page'):delta*=max(1,height//28)
                self.model.move_to(self.layout.vertical_target(self.model.cursor,delta),shift)
            elif key in ('Home','End') and not ctrl:self.model.move_to(self.layout.line_edge(self.model.cursor,key=='End'),shift)
            elif key in ('ArrowLeft','ArrowRight') and not ctrl:
                if self.model.cursor!=self.model.anchor and not shift:self.model.move_to(self.model.selected_range()[key=='ArrowRight'])
                else:self.model.move_to(self.layout.visual_target(self.model.cursor,1 if key=='ArrowRight' else -1),shift)
            else:self.layout.reset_navigation();changed=self.model.edit(request)
        elif kind=='command' and request.get('command')=='park':
            write_checkpoint(self.directory,self,self.opening)
        elif kind=='command' and request.get('command')=='captureNavigation':
            navigation=self.navigation_capture()
        elif kind=='command' and request.get('command')=='restoreNavigation':
            bookmark=request.get('bookmark')
            if not isinstance(bookmark,dict) or set(bookmark)!={'documentHash','cursor','anchor'}:raise ValueError('Invalid reading location.')
            if bookmark['documentHash']!=self.navigation_capture()['documentHash']:raise ValueError('This scene changed since that reading location. Its current caret and selection are preserved.')
            for key in ('cursor','anchor'):
                point=bookmark[key]
                if not isinstance(point,list) or len(point)!=2 or any(type(part) is not int for part in point) or not 0<=point[0]<len(self.model.blocks) or not 0<=point[1]<=len(self.model.block_text(point[0])):raise ValueError('Invalid reading location.')
            self.model.move_to(tuple(bookmark['anchor']))
            self.model.move_to(tuple(bookmark['cursor']),True)
            self.layout.reset_navigation();navigation=self.navigation_capture()
        elif kind=='command' and request.get('command')=='captureParagraph':
            capture=self.paragraph_capture()
        elif kind=='command' and request.get('command')=='swapParagraph':
            anchor=request.get('anchor')
            if not isinstance(anchor,dict) or not {'blockIndex','documentHash'}<=set(anchor) or set(anchor)-{'blockIndex','documentHash','block'}:raise ValueError('Invalid paragraph anchor.')
            current=self.paragraph_capture(anchor.get('blockIndex'))
            if current['documentHash']!=anchor['documentHash'] or ('block' in anchor and current['block']!=anchor['block']):raise ValueError('This scene changed. Capture the paragraph again before swapping alternatives.')
            changed=self.model.swap_paragraph(anchor['blockIndex'],current['block'],request.get('replacement'))
            self.layout.reset_navigation();capture=self.paragraph_capture(anchor['blockIndex'])
        elif kind=='command' and request.get('command')=='find':
            self.search_result=self.model.find(request.get('query'));self.layout.reset_navigation()
        elif kind=='command' and request.get('command')=='replace':
            changed,self.search_result=self.model.replace_selection(request.get('query'),request.get('replacement'));self.layout.reset_navigation()
        elif kind=='command' and request.get('command')=='unicodeToggle':
            changed,self.unicode_result=self.model.unicode_toggle();self.layout.reset_navigation()
        elif kind=='text' and inputs is not None:
            self.layout.reset_navigation()
            for text in inputs:changed=self.model.edit({'type':'text','text':text}) or changed
        elif kind not in ('open','snapshot','refresh'):
            self.layout.reset_navigation();changed=self.model.edit(request)
        if defer:
            return {'deferred':True,'timing':{'processingMs':(time.perf_counter()-started)*1000,'outputMs':0}}
        self.layout.prepare()
        applied=time.perf_counter();frame=self.layout.frame(top,height,follow);self.top=frame['top']
        frame['position']={'cursor':list(self.model.cursor),'anchor':list(self.model.anchor)}
        if capture is not None:frame['paragraphCapture']=capture
        if navigation is not None:frame['navigationCapture']=navigation
        if kind in ('open','snapshot') or not preview:frame['document']=self.model.document_snapshot()
        if self.search_result is not None:frame['searchResult']=self.search_result
        if self.unicode_result is not None:frame['unicodeResult']=self.unicode_result
        frame['timing']={'processingMs':(applied-started)*1000,'outputMs':(time.perf_counter()-applied)*1000}
        return frame

def main():
    directory=Path(sys.argv[1]).resolve();directory.mkdir(mode=0o700,parents=True,exist_ok=True)
    editor=None
    while True:
        line=sys.stdin.buffer.readline(2_100_001)
        if not line:break
        if len(line)>2_100_000 or not line.endswith(b'\n'):raise ValueError('Editor request too large.')
        try:
            request=json.loads(line)
            if request.get('type')=='resume':
                saved,model=read_checkpoint(directory)
                candidate=NativeEditor(directory,saved['opening'])
                model.max_document_bytes=candidate.model.max_document_bytes
                candidate.model=model
                candidate.layout=NativeLayout(model,saved['width'],candidate.layout.font_family)
                candidate.top=saved['top'];candidate.height=saved['height']
                frame=candidate.request({'type':'snapshot','top':candidate.top,'viewportHeight':candidate.height})
                editor=candidate
            elif request.get('type')=='open':
                candidate=NativeEditor(directory,request);frame=candidate.request(request,True);editor=candidate
            elif editor is None:raise ValueError('The native manuscript editor is not open.')
            else:frame=editor.request(request)
            print(json.dumps({'ok':True,'frame':frame},ensure_ascii=False,separators=(',',':')),flush=True)
        except Exception as error:print(json.dumps({'ok':False,'error':str(error)},ensure_ascii=False),flush=True)

if __name__=='__main__':
    try:main()
    except BaseException:
        traceback.print_exc();sys.exit(1)
