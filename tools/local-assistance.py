#!/usr/bin/env python3
"""Explicit, project-local model setup. Nothing downloads or starts at import."""
import argparse,hashlib,json,os,subprocess,tarfile,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
RUNTIME=ROOT/'runtime/local-assistance'
FILES=[{'name':'llama-b11146-bin-ubuntu-x64.tar.gz','url':'https://github.com/ggml-org/llama.cpp/releases/download/b11146/llama-b11146-bin-ubuntu-x64.tar.gz','size':16998357,'sha256':'c150306eb16b5ab696f76a8bdf810c35fd98a24e82158742e6fa28f420ff8410'}, {'name':'Qwen3-4B-Q4_K_M.gguf','url':'https://huggingface.co/Qwen/Qwen3-4B-GGUF/resolve/bc640142c66e1fdd12af0bd68f40445458f3869b/Qwen3-4B-Q4_K_M.gguf','size':2497280256,'sha256':'7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5'}]
MODELS={
 'qwen3-4b':{'name':'Qwen3-4B Q4_K_M','source':'https://huggingface.co/Qwen/Qwen3-4B-GGUF','file':FILES[1],'manifest':'manifest.json','evaluation':'Measured baseline; all four clean continuity controls produced false positives.'},
 'qwen3.5-4b':{'name':'Qwen3.5-4B Q4_K_M','source':'https://huggingface.co/lmstudio-community/Qwen3.5-4B-GGUF','baseModel':'https://huggingface.co/Qwen/Qwen3.5-4B/tree/851bf6e806efd8d0a36b00ddf55e13ccb7b8cd0a','quantizer':'LM Studio team, llama.cpp b8185','file':{'name':'Qwen3.5-4B-Q4_K_M.gguf','url':'https://huggingface.co/lmstudio-community/Qwen3.5-4B-GGUF/resolve/f9f88ac3e234be915e23811a6d28ea287bdb927e/Qwen3.5-4B-Q4_K_M.gguf','size':2707513696,'sha256':'25082a7dd3776cc3c741c6347d3bd04523f05796607b3fbc32fa3a25dfa1418c'},'manifest':'manifest-qwen3.5-4b.json','evaluation':'Candidate only: not downloaded or benchmarked. Runtime source supports qwen35; actual loading and quality remain unverified.'},
}
def checksum(path):
 h=hashlib.sha256()
 with path.open('rb') as source:
  for chunk in iter(lambda:source.read(1024*1024),b''):h.update(chunk)
 return h.hexdigest()
def download(spec):
 RUNTIME.mkdir(parents=True,exist_ok=True);path=RUNTIME/spec['name']
 if path.is_file() and path.stat().st_size==spec['size'] and checksum(path)==spec['sha256']:return path
 temporary=path.with_suffix(path.suffix+'.partial');h=hashlib.sha256();size=0
 with urllib.request.urlopen(spec['url'],timeout=60) as response,temporary.open('wb') as target:
  for chunk in iter(lambda:response.read(1024*1024),b''):
   size+=len(chunk)
   if size>spec['size']:raise ValueError('Download exceeded its pinned size')
   target.write(chunk);h.update(chunk)
  target.flush();os.fsync(target.fileno())
 if size!=spec['size'] or h.hexdigest()!=spec['sha256']:raise ValueError('Source size/checksum verification failed; partial file retained for diagnosis')
 temporary.replace(path);return path
def main():
 parser=argparse.ArgumentParser();parser.add_argument('--download',action='store_true');parser.add_argument('--serve',action='store_true');parser.add_argument('--model',choices=MODELS,default='qwen3-4b');options=parser.parse_args()
 model=MODELS[options.model];files=[FILES[0],model['file']];manifest_path=RUNTIME/model['manifest']
 if options.download:
  for spec in files:download(spec)
  destination=RUNTIME/'llama-b11146';destination.mkdir(exist_ok=True)
  with tarfile.open(RUNTIME/FILES[0]['name']) as archive:archive.extractall(destination,filter='data')
  manifest_path.write_text(json.dumps({'runtime':'llama.cpp b11146 CPU','model':model['name'],'modelLicense':'Apache-2.0','files':files,'context':4096,'threads':4,'source':model['source'],'evaluation':model['evaluation']},indent=2))
 if options.serve:
  if not manifest_path.is_file():raise RuntimeError('Download and verify the selected model explicitly before starting this local service')
  binaries=list((RUNTIME/'llama-b11146').rglob('llama-server'))
  if len(binaries)!=1:raise RuntimeError('Verified server executable is missing or ambiguous')
  executable=binaries[0];env=dict(os.environ,LD_LIBRARY_PATH=str(executable.parent))
  os.execvpe('/usr/bin/nice',['nice','-n','15',str(executable),'-m',str(RUNTIME/model['file']['name']),'--host','127.0.0.1','--port','8080','-c','4096','-t','4','-tb','4','-ngl','0','--parallel','1','--no-webui','--jinja'],env)
 if not options.download and not options.serve:print(json.dumps({'selectedModel':options.model,'model':model['name'],'modelLicense':'Apache-2.0','explicitDownloadBytes':sum(f['size'] for f in files),'modelDownloadBytes':model['file']['size'],'files':files,'source':model['source'],'evaluation':model['evaluation'],'notes':'Metadata only: no download, startup service, external listener or inference request is created. Downloading an unapproved candidate requires separate authorization.'},indent=2))
if __name__=='__main__':main()
