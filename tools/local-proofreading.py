#!/usr/bin/env python3
"""Explicit project-local LanguageTool setup; no download or listener at import."""
import argparse,hashlib,json,os,subprocess,urllib.request,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
RUNTIME=ROOT/'runtime/local-proofreading'
SPEC={'version':'6.6','url':'https://languagetool.org/download/LanguageTool-6.6.zip','bytes':251998221,'licenseReview':'Inspect included component licenses; LanguageTool core LGPL-2.1-or-later','publishedChecksum':None}
def main():
 parser=argparse.ArgumentParser();parser.add_argument('--download',action='store_true');parser.add_argument('--serve',action='store_true');options=parser.parse_args()
 if options.download:
  RUNTIME.mkdir(parents=True,exist_ok=True);archive=RUNTIME/'LanguageTool-6.6.zip';partial=archive.with_suffix('.zip.partial')
  request=urllib.request.Request(SPEC['url'],headers={'User-Agent':'NovelWorkspace-local-setup/0.1'})
  h=hashlib.sha256();size=0
  with urllib.request.urlopen(request,timeout=60) as response,partial.open('wb') as target:
   for chunk in iter(lambda:response.read(1024*1024),b''):
    size+=len(chunk)
    if size>SPEC['bytes']:raise ValueError('Official release exceeded expected size')
    target.write(chunk);h.update(chunk)
   target.flush();os.fsync(target.fileno())
  if size!=SPEC['bytes']:raise ValueError('Incomplete release download')
  with zipfile.ZipFile(partial) as source:
   if len(source.infolist())>10000 or sum(info.file_size for info in source.infolist())>1200000000:raise ValueError('Oversize package contents')
   for info in source.infolist():
    path=Path(info.filename)
    if path.is_absolute() or '..' in path.parts or (info.external_attr>>16)&0o170000==0o120000:raise ValueError('Unsafe package path')
   if source.testzip() is not None:raise ValueError('Package CRC validation failed')
   source.extractall(RUNTIME)
  partial.replace(archive)
  (RUNTIME/'manifest.json').write_text(json.dumps({**SPEC,'sha256':h.hexdigest(),'checksumMeaning':'Local integrity/provenance record; upstream does not advertise an authenticated checksum here'},indent=2))
 if options.serve:
  manifest=json.loads((RUNTIME/'manifest.json').read_text());archive=RUNTIME/'LanguageTool-6.6.zip'
  if hashlib.file_digest(archive.open('rb'),'sha256').hexdigest()!=manifest['sha256']:raise ValueError('Local release integrity check failed')
  home=RUNTIME/'LanguageTool-6.6';jar=home/'languagetool-server.jar'
  if not jar.is_file():raise ValueError('Local server package missing')
  # No --public, CORS wildcard, remote listener, persistent service or cloud rules.
  os.execvp('java',['java','-Xms64m','-Xmx512m','-cp',str(jar),'org.languagetool.server.HTTPServer','--port','8081'])
 if not options.download and not options.serve:print(json.dumps({**SPEC,'notes':'Nothing starts automatically. Serving is temporary loopback only. Download requires explicit authorization.'},indent=2))
if __name__=='__main__':main()
