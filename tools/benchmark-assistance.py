#!/usr/bin/env python3
"""Opt-in synthetic evaluation. Starts and stops only its own temporary server."""
import argparse,datetime,hashlib,json,os,re,signal,socket,subprocess,tempfile,threading,time,urllib.error,urllib.request,uuid
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
SCRATCH=ROOT.parent.parent/'work'
ENDPOINT='http://127.0.0.1:8080'
VALIDATOR="""import {parseModelSuggestions} from './apps/writer/src/assistance/model.ts';
let input='';for await(const chunk of process.stdin)input+=chunk;
try {const data=JSON.parse(input);const suggestions=parseModelSuggestions(data.content,data.sources);console.log(JSON.stringify({valid:true,suggestions}));}
catch(error){console.log(JSON.stringify({valid:false,error:error.message}));}"""

def production_instructions():
    # Exercise the application prompt itself, without a separately tuned rubric.
    code=(ROOT/'crates/companion/src/providers.rs').read_text()
    return re.search(r'let instructions="([^"\n]+)";',code).group(1)

def source(identity,text,kind='scene'):
    return {'sceneId':identity,'revisionId':'synthetic-revision-1','hash':hashlib.sha256(text.encode()).hexdigest(),'kind':kind,'text':text}

def fixtures():
    languages=[('en','Mira','At noon, Mira had green eyes.','At noon, Mira had brown eyes.','brown'),('ar','ميرا','عند الظهر، كانت عينا ميرا خضراوين.','عند الظهر، كانت عينا ميرا بنيتين.','بنيتين'),('zh-Hans','米拉','中午，米拉的眼睛是绿色的。','中午，米拉的眼睛是棕色的。','棕色'),('zh-Hant','米拉','中午，米拉的眼睛是綠色的。','中午，米拉的眼睛是棕色的。','棕色')]
    for language,name,clean,contradiction,wrong in languages:
        facts=json.dumps({'schemaVersion':1,'records':[{'id':'character-mira','name':name}],'facts':[{'id':'fact-eyes','subjectId':'character-mira','property':'eye-color','value':{'en':'green','ar':'خضراء','zh-Hans':'绿色','zh-Hant':'綠色'}[language],'eventId':None,'untilEventId':None}]},ensure_ascii=False,separators=(',',':'),sort_keys=True)
        for seeded,text in [(True,contradiction),(False,clean)]:
            yield {'id':language+('-seeded' if seeded else '-clean'),'language':language,'seededConflict':seeded,'conflictMarker':wrong,'sources':[source('synthetic-scene-'+language,text),source('planning:synthetic',facts,'facts')]}

def schema():
    string={'type':'string'}
    ref={'type':'object','additionalProperties':False,'required':['sceneId','revisionId','hash'],'properties':{key:string for key in ['sceneId','revisionId','hash']}}
    evidence={'type':'object','additionalProperties':False,'required':['sceneId','revisionId','hash','offsetUnit','start','end','quote','prefix','suffix'],'properties':{**ref['properties'],'offsetUnit':{'const':'utf16'},'start':{'type':'integer','minimum':0},'end':{'type':'integer','minimum':1},'quote':string,'prefix':string,'suffix':string}}
    action={'oneOf':[{'type':'object','additionalProperties':False,'required':['kind'],'properties':{'kind':{'const':'question'}}},{'type':'object','additionalProperties':False,'required':['kind','replacement'],'properties':{'kind':{'const':'replace'},'replacement':string}},{'type':'object','additionalProperties':False,'required':['kind','subject','property','value'],'properties':{'kind':{'const':'fact'},'subject':string,'property':string,'value':string}}]}
    properties={'id':string,'origin':{'const':'llm'},'title':string,'explanation':string,'status':{'const':'pending'},'sources':{'type':'array','items':ref,'minItems':1,'maxItems':2},'evidence':{'type':'array','items':evidence,'maxItems':4},'action':action}
    return {'type':'array','maxItems':4,'items':{'type':'object','additionalProperties':False,'required':list(properties),'properties':properties}}

def request(path,data=None,timeout=120):
    body=json.dumps(data,ensure_ascii=False).encode() if data is not None else None
    req=urllib.request.Request(ENDPOINT+path,body,{'Content-Type':'application/json'})
    with urllib.request.urlopen(req,timeout=timeout) as response:
        raw=response.read(2_000_001)
        if len(raw)>2_000_000:raise ValueError('Local response exceeds application limit')
        return json.loads(raw)

class NativeWorkspace:
    def __init__(self):
        self.directory=tempfile.TemporaryDirectory(prefix='model-evaluation-workspace-',dir=SCRATCH)
        self.process=subprocess.Popen([str(ROOT/'target/debug/novel-companion'),self.directory.name],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
        self.sequence=0
    def call(self,operation,args):
        self.sequence+=1;identity=self.sequence
        self.process.stdin.write(json.dumps({'id':identity,'op':operation,'args':args},ensure_ascii=False)+'\n');self.process.stdin.flush()
        reply=json.loads(self.process.stdout.readline())
        if reply.get('id')!=identity:raise RuntimeError('Unexpected synthetic companion response identity')
        if reply.get('ok') is not True:raise RuntimeError(reply.get('error','Synthetic companion request failed'))
        return reply['value']
    def sources(self,case):
        project=self.call('create_project',{'title':'Synthetic '+case['id']})
        chapter=self.call('create_chapter',{'projectId':project['id'],'title':'Synthetic chapter'})
        text=case['sources'][0]['text'];document={'type':'doc','content':[{'type':'paragraph','attrs':{'lang':case['language'],'dir':'rtl' if case['language']=='ar' else 'auto'},'content':[{'type':'text','text':text}]}]}
        scene=self.call('create_scene',{'chapterId':chapter['id'],'scene':{'schemaVersion':1,'title':'Synthetic passage','document':document}})
        canonical=json.loads(case['sources'][1]['text']);record=canonical['records'][0]
        planning={'schemaVersion':1,'fields':[],'records':[{**record,'kind':'character','aliases':[],'notes':'','values':{},'links':[]}],'plotPoints':[],'facts':[{**canonical['facts'][0],'status':'fact','confirmed':True,'observerId':None,'source':'Explicit synthetic fixture'}],'boards':[],'lanes':[{'id':'main-lane','name':'Main plot','kind':'main'}],'events':[],'constraints':[],'issues':[]}
        saved=self.call('save_project_data',{'projectId':project['id'],'kind':'planning','payload':planning,'parent':None})
        sources=[{**source(scene['id'],text),'revisionId':scene['revisionId'],'language':case['language']},{**source('planning:'+project['id'],case['sources'][1]['text'],'facts'),'revisionId':saved['revisionId'],'language':'und'}]
        return project['id'],sources,scene
    def close(self):
        if self.process.poll() is None:
            self.process.stdin.close()
            try:self.process.wait(timeout=15)
            except subprocess.TimeoutExpired:self.process.terminate();self.process.wait(timeout=15)
        self.process.stdout.close();self.directory.cleanup()

def evaluate_native(case,workspace):
    project,sources,before=workspace.sources(case);identity='evaluation-'+uuid.uuid4().hex
    request_body={'schemaVersion':1,'id':identity,'idempotencyKey':identity,'projectId':project,'provider':'llm','operation':'consistency','language':case['language'],'sources':sources,'contextBudget':4096,'timeoutMs':120000,'createdAt':datetime.datetime.now(datetime.timezone.utc).isoformat()}
    result={'fixture':{**case,'sources':sources},'mode':'native-production','request':request_body};start=time.monotonic()
    try:
        response=workspace.call('provider_run',{'endpoint':ENDPOINT,'model':'local','runToken':identity+':1','request':request_body})
        result['response']=response
        checked=subprocess.run(['node','--experimental-strip-types','--input-type=module','-e',VALIDATOR],cwd=ROOT,input=json.dumps({'content':json.dumps(response['suggestions']),'sources':sources}),text=True,capture_output=True,timeout=10,check=True)
        validation=json.loads(checked.stdout);result['validation']=validation;suggestions=validation.get('suggestions',[])
        result['seedDetected']=case['seededConflict'] and validation['valid'] and any(s['action']['kind']=='question' and any(a['sceneId']==sources[0]['sceneId'] and case['conflictMarker'] in a['quote'] for a in s['evidence']) for s in suggestions)
        result['falsePositive']=not case['seededConflict'] and validation['valid'] and bool(suggestions)
    except Exception as error:result['error']=str(error)
    result['wallSeconds']=round(time.monotonic()-start,3)
    after=workspace.call('load_scene',{'sceneId':before['id']})
    result['manuscriptUnchanged']=after['revisionId']==before['revisionId'] and after['scene']==before['scene']
    return result

def evaluate(case,mode,prompt):
    user={'operation':'consistency','language':case['language'],'sources':case['sources'],'selection':None}
    payload={'model':'local','messages':[{'role':'system','content':prompt},{'role':'user','content':json.dumps(user,ensure_ascii=False)}],'temperature':0.1,'max_tokens':2048,'stream':False,'seed':42}
    if mode=='constrained-no-thinking':
        payload.update(chat_template_kwargs={'enable_thinking':False},response_format={'type':'json_object','schema':schema()})
    start=time.monotonic();result={'fixture':case,'mode':mode,'request':payload}
    try:
        response=request('/v1/chat/completions',payload)
        result['response']=response
        content=response['choices'][0]['message'].get('content') or ''
        checked=subprocess.run(['node','--experimental-strip-types','--input-type=module','-e',VALIDATOR],cwd=ROOT,input=json.dumps({'content':content,'sources':case['sources']}),text=True,capture_output=True,timeout=10,check=True)
        validation=json.loads(checked.stdout);result['validation']=validation
        suggestions=validation.get('suggestions',[])
        # Valid structure is necessary but not a semantic success. A seeded
        # hit must question/replace the contradictory passage with exact evidence.
        result['seedDetected']=case['seededConflict'] and validation['valid'] and any(s['action']['kind'] in ['question','replace'] and any(a['sceneId']==case['sources'][0]['sceneId'] and case['conflictMarker'] in a['quote'] for a in s['evidence']) for s in suggestions)
        result['falsePositive']=not case['seededConflict'] and validation['valid'] and bool(suggestions)
    except Exception as error:result['error']=str(error)
    result['wallSeconds']=round(time.monotonic()-start,3)
    return result

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--run',action='store_true',help='Explicitly start the downloaded model and run synthetic evaluation');parser.add_argument('--modes',nargs='+',choices=['production','constrained-no-thinking','native-production'],default=['native-production']);parser.add_argument('--model',choices=['qwen3-4b','qwen3.5-4b'],default='qwen3-4b');args=parser.parse_args()
    if not args.run:parser.error('--run is required; importing or inspecting this script starts nothing')
    def interrupted(signum,frame):raise KeyboardInterrupt('Benchmark interrupted; its own server will stop')
    signal.signal(signal.SIGTERM,interrupted)
    # Never share, replace, signal, or adopt an existing user's listener.
    with socket.socket() as probe:probe.bind(('127.0.0.1',8080))
    runtime=ROOT/'runtime/local-assistance';manifest=json.loads((runtime/('manifest.json' if args.model=='qwen3-4b' else 'manifest-qwen3.5-4b.json')).read_text())
    binaries=list((runtime/'llama-b11146').rglob('llama-server'))
    if len(binaries)!=1:raise RuntimeError('Verified llama-server is unavailable')
    binary=binaries[0];environment=dict(os.environ,LD_LIBRARY_PATH=str(binary.parent))
    version=subprocess.check_output([str(binary),'--version'],env=environment,stderr=subprocess.STDOUT,text=True)
    SCRATCH.mkdir(exist_ok=True);stamp=datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ');raw_path=SCRATCH/('model-evaluation-'+stamp+'.json');log_path=SCRATCH/('model-evaluation-'+stamp+'.log')
    report={'startedUtc':stamp,'manifest':manifest,'runtimeVersion':version,'hardware':{'cpu':next((line.split(':',1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')),'unknown'),'memoryKiB':next(line for line in Path('/proc/meminfo').read_text().splitlines() if line.startswith('MemTotal'))},'configuration':{'host':'127.0.0.1','port':8080,'context':4096,'parallelSlots':1,'threads':4,'nice':15,'gpuLayers':0},'results':[]}
    peak={'rssKiB':0,'hwmKiB':0};stop=threading.Event();process=None;workspace=None;prompt=production_instructions() if any(mode!='native-production' for mode in args.modes) else None
    def sample():
        while not stop.wait(0.25):
            try:
                status=Path(f'/proc/{process.pid}/status').read_text()
                for label,key in [('VmRSS','rssKiB'),('VmHWM','hwmKiB')]:
                    match=re.search(r'^'+label+r':\s+(\d+)',status,re.M)
                    if match:peak[key]=max(peak[key],int(match.group(1)))
            except FileNotFoundError:return
    try:
        with log_path.open('w') as log:
            process=subprocess.Popen(['/usr/bin/python3',str(ROOT/'tools/local-assistance.py'),'--serve','--model',args.model],stdout=log,stderr=log,cwd=ROOT,env=dict(os.environ,LLAMA_ARG_N_PARALLEL='1'))
            report['ownServerPid']=process.pid;thread=threading.Thread(target=sample,daemon=True);thread.start();started=time.monotonic()
            while True:
                if process.poll() is not None:raise RuntimeError('Temporary server exited: '+log_path.read_text()[-4000:])
                try:
                    if request('/health',timeout=2).get('status')=='ok':break
                except (OSError,urllib.error.URLError):pass
                if time.monotonic()-started>120:raise TimeoutError('Temporary server did not become ready within 120 seconds')
                time.sleep(0.5)
            report['loadSeconds']=round(time.monotonic()-started,3)
            if 'native-production' in args.modes:workspace=NativeWorkspace()
            if 'constrained-no-thinking' in args.modes:
                probe=request('/v1/chat/completions',{'model':'local','messages':[{'role':'user','content':'Return {"wrong":true}.'}],'chat_template_kwargs':{'enable_thinking':False},'response_format':{'type':'json_object','schema':{'type':'object','additionalProperties':False,'required':['schemaVerified'],'properties':{'schemaVerified':{'const':True}}}},'max_tokens':32,'temperature':0,'stream':False})
                report['schemaPreflight']=probe
                if json.loads(probe['choices'][0]['message']['content'])!={'schemaVerified':True}:raise RuntimeError('The runtime did not enforce the evaluation JSON schema')
            for mode in args.modes:
                for case in fixtures():
                    result=evaluate_native(case,workspace) if mode=='native-production' else evaluate(case,mode,prompt);report['results'].append(result)
                    raw_path.write_text(json.dumps(report,ensure_ascii=False,indent=2))
                    print(json.dumps({'fixture':case['id'],'mode':mode,'seconds':result['wallSeconds'],'valid':result.get('validation',{}).get('valid',False),'seedDetected':result.get('seedDetected',False),'falsePositive':result.get('falsePositive',False),'error':result.get('error')},ensure_ascii=False),flush=True)
    except Exception as error:report['fatalError']=str(error);raise
    finally:
        stop.set()
        if workspace is not None:workspace.close()
        if process is not None and process.poll() is None:
            process.terminate()
            try:process.wait(timeout=15)
            except subprocess.TimeoutExpired:process.kill();process.wait(timeout=15)
        report['memoryPeak']=peak;report['ownServerStopped']=process is None or process.poll() is not None
        raw_path.write_text(json.dumps(report,ensure_ascii=False,indent=2))
        print('Raw synthetic evaluation: '+str(raw_path),flush=True)

if __name__=='__main__':main()
