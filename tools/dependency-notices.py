#!/usr/bin/env python3
"""Collect installed upstream notices without downloads or dependency changes.

The Rust inventory conservatively includes non-dev runtime/build dependencies,
not a claim that every listed crate is linked into the executable. Missing
upstream notice files are reported; metadata is not a legal approval.
"""
import hashlib,json,os,subprocess,shutil
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def main():
 cargo=shutil.which('cargo') or str(Path(os.environ.get('CARGO_HOME',Path.home()/'.cargo'))/'bin/cargo')
 metadata=json.loads(subprocess.check_output([cargo,'metadata','--offline','--locked','--format-version','1','--filter-platform','x86_64-unknown-linux-gnu'],cwd=ROOT))
 packages={p['id']:p for p in metadata['packages']};nodes={n['id']:n for n in metadata['resolve']['nodes']}
 pending=[p['id'] for p in packages.values() if p['name'] in ['novel-native-writer','novel-editor','novel-companion']];seen=set()
 while pending:
  key=pending.pop()
  if key in seen:continue
  seen.add(key);pending.extend(dep['pkg'] for dep in nodes[key]['deps'] if any(kind['kind']!='dev' for kind in dep['dep_kinds']))
 upstream=ROOT/'docs/third-party/upstream';supplemental=json.loads((upstream/'manifest.json').read_text())
 inventory=[];texts=['Third-party dependency notices\n\nCollected from installed locked dependencies and checksum-pinned upstream notices. Rust includes conservative runtime/build metadata; listed packages are not a binary payload map. External system runtimes, fonts, model weights and language packs are not redistributed by this file.\n']
 def collect(ecosystem,name,version,license_name,source,folder,license_file=None,shared_notice_folder=None):
  paths=[f for f in folder.rglob('*') if f.is_file() and not f.is_symlink() and len(f.relative_to(folder).parts)<=3 and (f.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','UNLICENSE','COPYRIGHT')) or any(part.upper() in {'LICENSES','LICENCES'} for part in f.relative_to(folder).parts[:-1]))]
  shared_paths=[]
  if shared_notice_folder:
   shared_paths=[f for f in shared_notice_folder.rglob('*') if f.is_file() and not f.is_symlink() and len(f.relative_to(shared_notice_folder).parts)<=3 and (f.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','UNLICENSE','COPYRIGHT')) or any(part.upper() in {'LICENSES','LICENCES'} for part in f.relative_to(shared_notice_folder).parts[:-1]))]
  if license_file:
   explicit=(folder/license_file).resolve()
   if explicit.is_relative_to(folder.resolve()) and explicit.is_file():paths.append(explicit)
  notices=[]
  for file in sorted(set(paths+shared_paths)):
   if file.stat().st_size>1_000_000:raise ValueError('Unexpectedly large upstream notice')
   content=file.read_text(encoding='utf-8')
   relative=file.relative_to(shared_notice_folder) if shared_notice_folder and file in shared_paths else file.relative_to(folder)
   if shared_notice_folder and file in shared_paths: relative=Path('vendor/wordcraft')/relative
   notices.append(str(relative))
   texts.append(f'\n===== {ecosystem}: {name} {version} — {relative} =====\n'+content+'\n')
  upstream_records=[]
  if not notices:
   for record in supplemental:
    if ecosystem!='cargo' or record['package']!=name:continue
    vcs=json.loads((folder/'.cargo_vcs_info.json').read_text())['git']['sha1']
    if record.get('sourceCommit',vcs)!=vcs or (record.get('sourceCommit') is None and '/'+vcs+'/' not in record['url']):raise ValueError('Supplemental notice does not match the installed source commit')
    file=upstream/record['file'];data=file.read_bytes()
    if hashlib.sha256(data).hexdigest()!=record['sha256']:raise ValueError('Supplemental notice checksum mismatch')
    upstream_records.append(record)
    texts.append(f'\n===== {ecosystem}: {name} {version} — {record["url"]} =====\n'+record.get('notice','')+'\n'+data.decode('utf-8')+'\n')
    if record.get('sourceUrl'):texts.append('Unmodified upstream source: '+record['sourceUrl']+'\n')
  inventory.append({'ecosystem':ecosystem,'name':name,'version':version,'declaredLicense':license_name,'source':source,'noticeFiles':notices,'upstreamNotices':upstream_records})
 for key in sorted(seen,key=lambda key:(packages[key]['name'],packages[key]['version'])):
  p=packages[key]
  manifest=Path(p['manifest_path']).resolve()
  vendored_root=(ROOT/'vendor/wordcraft').resolve()
  is_wordcraft=manifest.is_relative_to(vendored_root) and p['name'].startswith('wordcraft-')
  if p['source'] or is_wordcraft:
   collect('cargo',p['name'],p['version'],p['license'],p['repository'] or ('https://github.com/storytold/wordcraft' if is_wordcraft else None),Path(p['manifest_path']).parent,p.get('license_file'),vendored_root if is_wordcraft else None)
 destination=ROOT/'docs/third-party';destination.mkdir(exist_ok=True)
 (destination/'NOTICES.txt').write_text(''.join(texts))
 missing=[p['ecosystem']+':'+p['name']+'@'+p['version'] for p in inventory if not p['noticeFiles'] and not p['upstreamNotices']]
 (destination/'inventory.json').write_text(json.dumps({'scope':'Installed locked non-dev runtime/build dependency metadata; conservative list, not linked binary inventory.','cargoLockSha256':hashlib.sha256((ROOT/'Cargo.lock').read_bytes()).hexdigest(),'npmLockSha256':hashlib.sha256((ROOT/'package-lock.json').read_bytes()).hexdigest(),'packages':inventory,'missingNoticeFiles':missing},indent=2)+'\n')
 print(json.dumps({'packages':len(inventory),'missingNoticeFiles':missing,'noticeBytes':(destination/'NOTICES.txt').stat().st_size}))
 if missing:raise ValueError('Missing upstream notices: '+', '.join(missing))
if __name__=='__main__':main()
