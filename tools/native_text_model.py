"""Exact local manuscript editing, independent of shaping, storage and services."""
from bisect import bisect_left,bisect_right
from copy import deepcopy
import json
import regex

MARKS=('bold','italic','underline')
LANGUAGES=('en','ar','zh-Hans','zh-Hant','und')
SPACING=(1,1.15,1.5,2)

def validate_document(document):
    if not isinstance(document,dict) or document.get('type')!='doc' or set(document)-{'type','content'}:raise ValueError('Unsupported manuscript root.')
    blocks=document.get('content',[])
    if not isinstance(blocks,list):raise ValueError('Invalid manuscript paragraphs.')
    for block in blocks:
        if not isinstance(block,dict) or block.get('type') not in ('paragraph','heading') or set(block)-{'type','attrs','content'}:raise ValueError('Unsupported manuscript paragraph.')
        attrs=block.get('attrs',{})
        if not isinstance(attrs,dict) or set(attrs)-{'dir','lang','level','lineSpacing'}:raise ValueError('Unsupported paragraph metadata.')
        if attrs.get('dir','auto') not in ('auto','ltr','rtl') or attrs.get('lang','en') not in LANGUAGES:raise ValueError('Unsupported paragraph language or direction.')
        if 'level' in attrs and (block['type']!='heading' or type(attrs['level']) is not int or not 1<=attrs['level']<=6):raise ValueError('Unsupported heading level.')
        if 'lineSpacing' in attrs and (type(attrs['lineSpacing']) not in (int,float) or attrs['lineSpacing'] not in SPACING):raise ValueError('Unsupported line spacing.')
        content=block.get('content',[])
        if not isinstance(content,list):raise ValueError('Invalid manuscript runs.')
        for node in content:
            if not isinstance(node,dict):raise ValueError('Invalid manuscript run.')
            if node.get('type')=='hardBreak':
                if set(node)!={'type'}:raise ValueError('Unsupported line break metadata.')
            elif node.get('type')=='text':
                if set(node)-{'type','text','marks'} or not isinstance(node.get('text'),str):raise ValueError('Unsupported manuscript text.')
                validate_text(node['text'])
                marks=node.get('marks',[])
                if not isinstance(marks,list) or any(not isinstance(mark,dict) or set(mark)!={'type'} or mark.get('type') not in MARKS for mark in marks):raise ValueError('Unsupported manuscript formatting.')
                if len({mark['type'] for mark in marks})!=len(marks):raise ValueError('Duplicate manuscript formatting.')
            else:raise ValueError('Unsupported manuscript run.')

def validate_text(text,limit=2_000_000):
    if not isinstance(text,str) or len(text.encode('utf-8'))>limit or any(ord(char)<32 and char not in '\n\r\t' for char in text):raise ValueError('Unsupported text input. The manuscript was preserved.')

def node_text(node):return '\n' if node['type']=='hardBreak' else node['text']
def text_of(block):return ''.join(node_text(node) for node in block.get('content',[]))
def marked(text,marks):
    result={'type':'text','text':text}
    if marks:result['marks']=[{'type':mark} for mark in MARKS if mark in marks]
    return result
def compact(nodes):
    output=[]
    for node in nodes:
        if node['type']=='text' and not node['text']:continue
        if output and node['type']=='text' and output[-1]['type']=='text' and output[-1].get('marks',[])==node.get('marks',[]):
            output[-1]={**output[-1],'text':output[-1]['text']+node['text']}
        else:output.append(node)
    return output
def slice_runs(block,start,end):
    output=[];offset=0
    for node in block.get('content',[]):
        text=node_text(node);stop=offset+len(text)
        left=max(start,offset);right=min(end,stop)
        if right>left:
            if left==offset and right==stop:output.append(node)
            elif node['type']=='text':output.append({**node,'text':text[left-offset:right-offset]})
        offset=stop
    return output
def with_runs(block,nodes):
    result={**block};nodes=compact(nodes)
    if nodes:result['content']=nodes
    else:result.pop('content',None)
    return result

class TextModel:
    """Copy only changed blocks; immutable snapshots share untouched paragraphs.

    Undo retains at most 512 operations and approximately 64 MiB of copied text
    plus paragraph references. Saved SQLite revision history is separate.
    """
    def __init__(self,document):
        validate_document(document)
        self._original=deepcopy(document);self._original_blocks=tuple(self._original.get('content',[]))
        self.blocks=self._original_blocks or ({'type':'paragraph'},)
        self.cursor=(0,0);self.anchor=self.cursor;self.typing_marks=None
        self.undo=[];self.redo=[];self.undo_bytes=0;self.undo_trimmed=False;self.version=0
        self._texts={};self._graphemes={};self._wire_sizes={};self.max_document_bytes=None
        self._unicode_result=None
    def block_text(self,index):
        block=self.blocks[index];key=id(block)
        if key not in self._texts:self._texts[key]=(block,text_of(block))
        return self._texts[key][1]
    def block_attrs(self,index):return self.blocks[index].get('attrs',{})
    def boundaries(self,index):
        block=self.blocks[index];key=id(block)
        if key not in self._graphemes:self._graphemes[key]=(block,[0]+[match.end() for match in regex.finditer(r'\X',self.block_text(index))])
        return self._graphemes[key][1]
    def clamp(self,position):
        block=max(0,min(len(self.blocks)-1,int(position[0])));offset=max(0,min(len(self.block_text(block)),int(position[1])))
        return (block,offset)
    def move_to(self,position,extend=False):
        self._unicode_result=None
        self.cursor=self.clamp(position)
        if not extend:self.anchor=self.cursor
        self.typing_marks=None
    def selected_range(self):return tuple(sorted((self.cursor,self.anchor)))
    def selection_text(self):
        start,end=self.selected_range()
        if start==end:return ''
        return '\n'.join(self.block_text(i)[start[1] if i==start[0] else 0:end[1] if i==end[0] else None] for i in range(start[0],end[0]+1))
    def marks_at(self,position=None,prefer_left=True):
        block,offset=position or self.cursor;at=0;previous=set()
        for node in self.blocks[block].get('content',[]):
            stop=at+len(node_text(node));marks={mark['type'] for mark in node.get('marks',[])}
            if offset==at and at>0 and prefer_left:return previous
            if at<=offset<stop:return marks
            at=stop;previous=marks
        return previous
    def document_snapshot(self):
        # A wholly empty root remains wholly empty until an actual manuscript edit.
        if not self._original_blocks and len(self.blocks)==1 and not self.block_text(0) and self.blocks[0]=={'type':'paragraph'}:return deepcopy(self._original)
        return {'type':'doc','content':deepcopy(list(self.blocks))} if 'content' in self._original or self.blocks else deepcopy(self._original)
    def _state(self):return (self.blocks,self.cursor,self.anchor,None if self.typing_marks is None else frozenset(self.typing_marks))
    def _restore(self,state):
        self.blocks,self.cursor,self.anchor,self.typing_marks=state;self.typing_marks=None if self.typing_marks is None else set(self.typing_marks);self.version+=1
    def _record(self,before):
        if before==self._state():return False
        if self.max_document_bytes is not None:
            try:
                if self.wire_size()>self.max_document_bytes:raise ValueError('Scene exceeds the 2 MB limit. The manuscript was preserved.')
            except Exception:
                self._restore(before);self.version-=1;self.prune_caches();raise
        # References are cheap. Count copied paragraphs conservatively at four
        # bytes per code point without allocating UTF-8 copies per keystroke.
        current_ids={id(block) for block in self.blocks}
        cost=len(before[0])*8+sum(len(text_of(block))*4 for block in before[0] if id(block) not in current_ids)
        self.undo.append((before,cost));self.undo_bytes+=cost;self.redo.clear()
        while len(self.undo)>512 or self.undo_bytes>64*1024*1024:
            _,discarded=self.undo.pop(0);self.undo_bytes-=discarded;self.undo_trimmed=True
        self.version+=1
        self.prune_caches(current_ids)
        return before[0]!=self.blocks
    def prune_caches(self,current_ids=None):
        current_ids=current_ids if current_ids is not None else {id(block) for block in self.blocks}
        self._texts={key:item for key,item in self._texts.items() if key in current_ids}
        self._graphemes={key:item for key,item in self._graphemes.items() if key in current_ids}
        self._wire_sizes={key:item for key,item in self._wire_sizes.items() if key in current_ids}
    def wire_size(self):
        size=len(b'{"type":"doc","content":[]}')+max(0,len(self.blocks)-1)
        for block in self.blocks:
            key=id(block)
            if key not in self._wire_sizes:self._wire_sizes[key]=(block,len(json.dumps(block,ensure_ascii=False,separators=(',',':')).encode('utf-8')))
            size+=self._wire_sizes[key][1]
        return size
    def _replace(self,text):
        start,end=self.selected_range();first=self.blocks[start[0]];last=self.blocks[end[0]]
        prefix=slice_runs(first,0,start[1]);suffix=slice_runs(last,end[1],len(self.block_text(end[0])))
        marks=self.typing_marks if self.typing_marks is not None else self.marks_at(start,start==end)
        parts=text.split('\n');new=[]
        if len(parts)==1:
            new=[with_runs(first,prefix+([marked(parts[0],marks)] if parts[0] else [])+suffix)]
            caret=(start[0],start[1]+len(parts[0]))
        else:
            new.append(with_runs(first,prefix+([marked(parts[0],marks)] if parts[0] else [])))
            for part in parts[1:]:
                template={'type':'paragraph'}
                if first.get('attrs'):template['attrs']={key:value for key,value in first['attrs'].items() if key!='level'}
                new.append(with_runs(template,[marked(part,marks)] if part else []))
            new[-1]=with_runs(new[-1],new[-1].get('content',[])+suffix);caret=(start[0]+len(new)-1,len(parts[-1]))
        self.blocks=self.blocks[:start[0]]+tuple(new)+self.blocks[end[0]+1:];self.cursor=caret;self.anchor=caret
    def insert(self,text):
        validate_text(text);before=self._state()
        if text or self.cursor!=self.anchor:self._replace(text)
        return self._record(before)
    def insert_break(self):
        before=self._state();self._replace('');block,offset=self.cursor;node=self.blocks[block]
        changed=with_runs(node,slice_runs(node,0,offset)+[{'type':'hardBreak'}]+slice_runs(node,offset,len(self.block_text(block))))
        self.blocks=self.blocks[:block]+(changed,)+self.blocks[block+1:];self.cursor=(block,offset+1);self.anchor=self.cursor
        return self._record(before)
    def logical_target(self,forward,word=False):
        block,offset=self.cursor;text=self.block_text(block)
        if forward and offset==len(text):return (block+1,0) if block+1<len(self.blocks) else self.cursor
        if not forward and offset==0:return (block-1,len(self.block_text(block-1))) if block else self.cursor
        if word:
            matches=list(regex.finditer(r'[\p{L}\p{N}\p{M}_]+|\X',text))
            if forward:
                candidates=[match for match in matches if match.end()>offset and not match.group().isspace()]
                if not candidates:return (block,len(text))
                end=candidates[0].end()
                while end<len(text) and text[end].isspace():end+=1
                return (block,end)
            end=offset
            while end and text[end-1].isspace():end-=1
            candidates=[match for match in matches if match.start()<end]
            return (block,candidates[-1].start() if candidates else 0)
        boundaries=self.boundaries(block)
        return (block,boundaries[min(len(boundaries)-1,bisect_right(boundaries,offset))] if forward else boundaries[max(0,bisect_left(boundaries,offset)-1)])
    def delete(self,forward=False,word=False):
        before=self._state()
        if self.cursor==self.anchor:self.anchor=self.logical_target(forward,word)
        if self.cursor!=self.anchor:self._replace('')
        return self._record(before)
    def select_word(self,position):
        block,offset=self.clamp(position);text=self.block_text(block)
        for match in regex.finditer(r'[\p{L}\p{N}\p{M}_]+|\X',text):
            if match.start()<=offset<match.end() or offset==len(text)==match.end():
                self.anchor=(block,match.start());self.cursor=(block,match.end());self.typing_marks=None;return
        self.move_to((block,offset))
    def select_paragraph(self,position):
        block,_=self.clamp(position);self.anchor=(block,0);self.cursor=(block,len(self.block_text(block)));self.typing_marks=None
    def _selected_blocks(self):
        start,end=self.selected_range();last=end[0]-int(end[1]==0 and end[0]>start[0]);return range(start[0],last+1)
    def paragraph_format(self,**attrs):
        before=self._state();blocks=list(self.blocks)
        for index in self._selected_blocks():
            block=blocks[index];properties={**block.get('attrs',{})};kind=attrs.get('style',block['type'])
            properties.update({key:value for key,value in attrs.items() if key!='style'})
            if kind=='heading':properties.setdefault('level',2)
            else:properties.pop('level',None)
            changed={**block,'type':kind,'attrs':properties}
            blocks[index]=changed if changed!=block else block
        self.blocks=tuple(blocks);return self._record(before)
    def character_format(self,mark=None,clear=False):
        if mark is not None and mark not in MARKS:raise ValueError('Unsupported formatting command.')
        before=self._state()
        if self.cursor==self.anchor:
            marks=set(self.typing_marks if self.typing_marks is not None else self.marks_at())
            if clear:marks.clear()
            elif mark in marks:marks.remove(mark)
            else:marks.add(mark)
            self.typing_marks=marks;self._record(before);return False
        start,end=self.selected_range();selected=[]
        for index in range(start[0],end[0]+1):selected.extend(slice_runs(self.blocks[index],start[1] if index==start[0] else 0,end[1] if index==end[0] else len(self.block_text(index))))
        remove=not clear and all(mark in {m['type'] for m in node.get('marks',[])} for node in selected if node['type']=='text')
        blocks=list(self.blocks)
        for index in range(start[0],end[0]+1):
            block=blocks[index];left=start[1] if index==start[0] else 0;right=end[1] if index==end[0] else len(self.block_text(index));changed=[]
            for node in slice_runs(block,left,right):
                if node['type']=='hardBreak':changed.append(node);continue
                marks={m['type'] for m in node.get('marks',[])}
                if clear:marks.clear()
                elif remove:marks.discard(mark)
                else:marks.add(mark)
                changed.append(marked(node['text'],marks))
            candidate=with_runs(block,slice_runs(block,0,left)+changed+slice_runs(block,right,len(self.block_text(index))))
            blocks[index]=candidate if candidate!=block else block
        self.blocks=tuple(blocks);return self._record(before)
    def flat_text(self):return '\n'.join(self.block_text(index) for index in range(len(self.blocks)))
    def flat_offset(self,position):return sum(len(self.block_text(index))+1 for index in range(position[0]))+position[1]
    def from_flat(self,offset):
        for index in range(len(self.blocks)):
            size=len(self.block_text(index))
            if offset<=size:return (index,max(0,offset))
            offset-=size+1
        return (len(self.blocks)-1,len(self.block_text(len(self.blocks)-1)))
    def find(self,query):
        if not isinstance(query,str) or not query or len(query)>1000 or any(ord(char)<32 for char in query):raise ValueError('Enter 1–1000 characters without control characters to find.')
        self._unicode_result=None
        text=self.flat_text();start=self.flat_offset(self.selected_range()[1]);found=text.find(query,start);wrapped=False
        if found<0:found=text.find(query);wrapped=found>=0
        if found>=0:self.anchor=self.from_flat(found);self.cursor=self.from_flat(found+len(query));self.typing_marks=None
        return {'found':found>=0,'wrapped':wrapped}
    def replace_selection(self,query,replacement):
        if not isinstance(query,str) or not query or len(query)>1000 or any(ord(char)<32 for char in query):raise ValueError('Enter 1–1000 characters without control characters to find.')
        if not isinstance(replacement,str) or len(replacement)>10000 or any(ord(char)<32 for char in replacement):raise ValueError('Replacement must contain at most 10000 characters without control characters.')
        if self.selection_text()!=query:return False,{'found':False,'replaced':False,'message':'Find the text again before replacing.'}
        return self.insert(replacement),{'found':True,'replaced':True}
    def unicode_toggle(self):
        start,end=self.selected_range();selected=self.selection_text()
        reverse_result=self._unicode_result==(start,end,selected,'character')
        if not selected:
            block,offset=self.cursor
            if not offset:return False,{'converted':False,'message':'Select a character or hexadecimal Unicode code.'}
            text=self.block_text(block)[:offset];match=regex.search(r'[0-9A-Fa-f]+$',text)
            if match:start=(block,match.start());selected=match.group()
            else:start=(block,offset-1);selected=text[-1]
            end=self.cursor
        if regex.fullmatch(r'[0-9A-Fa-f]+',selected) and not reverse_result:
            if len(selected)>6:return False,{'converted':False,'message':'Unicode hexadecimal codes contain at most six digits.'}
            scalar=int(selected,16)
            if scalar>0x10FFFF or 0xD800<=scalar<=0xDFFF:return False,{'converted':False,'message':'Enter a valid Unicode scalar value.'}
            replacement=chr(scalar)
            try:validate_text(replacement)
            except ValueError:return False,{'converted':False,'message':'This control character cannot be inserted into the manuscript.'}
        elif len(selected)==1:replacement=f'{ord(selected):04X}'
        else:return False,{'converted':False,'message':'Select one character or a hexadecimal Unicode code.'}
        before=self._state();self.anchor=start;self.cursor=end;self._replace(replacement);self.anchor=start
        self._unicode_result=(start,self.cursor,replacement,'character' if len(replacement)==1 else 'hex')
        return self._record(before),{'converted':True,'text':replacement}
    def history(self,redo=False):
        self._unicode_result=None
        source=self.redo if redo else self.undo;destination=self.undo if redo else self.redo
        if not source:return False
        previous=self._state();state,cost=source.pop();destination.append((previous,cost))
        self.undo_bytes+=cost if redo else -cost;self._restore(state);self.prune_caches();return previous[0]!=self.blocks
    def states(self):
        marks=self.typing_marks if self.typing_marks is not None else self.marks_at()
        return {name:'true' if name in marks else 'false' for name in MARKS}|{'undo':'true' if self.undo else 'false','redo':'true' if self.redo else 'false','undoTrimmed':'true' if self.undo_trimmed else 'false','undoDepth':str(len(self.undo))}
    def swap_paragraph(self,index,expected,replacement):
        """Replace one structurally anchored block as one native undo action."""
        if type(index) is not int or not 0<=index<len(self.blocks) or self.blocks[index]!=expected:
            raise ValueError('This paragraph changed. Capture it again before swapping alternatives.')
        validate_document({'type':'doc','content':[replacement]})
        if replacement==self.blocks[index]:return False
        before=self._state();block=deepcopy(replacement)
        self.blocks=self.blocks[:index]+(block,)+self.blocks[index+1:]
        self.cursor=self.clamp(self.cursor);self.anchor=self.cursor;self.typing_marks=None
        self._unicode_result=None
        return self._record(before)

    def edit(self,request):
        kind=request.get('type')
        self._unicode_result=None
        if kind=='text':return self.insert(request.get('text',''))
        if kind=='key':
            key=request.get('key');shift=bool(request.get('shift'));ctrl=bool(request.get('ctrl'))
            if key in ('ArrowLeft','ArrowRight'):
                if self.cursor!=self.anchor and not shift and not ctrl:self.move_to(self.selected_range()[key=='ArrowRight'])
                else:self.move_to(self.logical_target(key=='ArrowRight',ctrl),shift)
                return False
            if key in ('Backspace','Delete'):return self.delete(key=='Delete',ctrl)
            if key=='Enter':return self.insert_break() if shift else self.insert('\n')
            if key=='Tab':return self.insert('\t')
            if key=='Home':self.move_to((0,0) if ctrl else (self.cursor[0],0),shift);return False
            if key=='End':self.move_to((len(self.blocks)-1,len(self.block_text(len(self.blocks)-1))) if ctrl else (self.cursor[0],len(self.block_text(self.cursor[0]))),shift);return False
            if key=='Escape':self.anchor=self.cursor;return False
            raise ValueError('Unsupported manuscript key.')
        if kind=='command':
            command=request.get('command')
            if command in MARKS:return self.character_format(command)
            if command=='clearFormatting':return self.character_format(clear=True)
            if command in ('undo','redo'):return self.history(command=='redo')
            if command=='selectAll':self.anchor=(0,0);self.cursor=(len(self.blocks)-1,len(self.block_text(len(self.blocks)-1)));return False
            if command in ('ltr','rtl'):return self.paragraph_format(dir=command)
            if command=='lineSpacing':
                value=request.get('lineSpacing')
                if type(value) not in (int,float) or value not in SPACING:raise ValueError('Choose line spacing 1, 1.15, 1.5, or 2.')
                return self.paragraph_format(lineSpacing=value)
            if command=='blockStyle':
                language=request.get('language');direction=request.get('direction');style=request.get('style')
                if language not in LANGUAGES or direction not in ('auto','ltr','rtl') or style not in ('paragraph','heading'):raise ValueError('Choose an explicit language, direction and paragraph style.')
                return self.paragraph_format(lang=language,dir=direction,style=style)
            raise ValueError('Unsupported manuscript command.')
        if kind in ('snapshot','refresh'):return False
        raise ValueError('Unsupported manuscript operation.')
