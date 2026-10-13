#!/usr/bin/env python3
"""Detached personal-build signature. Keys must be outside the source repository."""
import argparse,os,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def main():
 p=argparse.ArgumentParser();p.add_argument('package',type=Path);p.add_argument('--key-directory',type=Path,required=True);p.add_argument('--public-key',type=Path,required=True);a=p.parse_args();directory=a.key_directory.resolve()
 if directory==ROOT or ROOT in directory.parents:raise ValueError('Keep the private signing key outside this repository')
 if not a.package.is_file():raise ValueError('Build the package first')
 directory.mkdir(mode=0o700,parents=True,exist_ok=True);directory.chmod(0o700)
 base=['gpg','--homedir',str(directory),'--batch','--no-tty']
 keys=subprocess.run(base+['--with-colons','--list-secret-keys'],text=True,capture_output=True,check=True).stdout
 if not any(line.startswith('sec:') for line in keys.splitlines()):subprocess.run(base+['--pinentry-mode','loopback','--passphrase','','--quick-generate-key','Novel Workspace local personal build','ed25519','sign','0'],check=True)
 keys=subprocess.run(base+['--with-colons','--list-secret-keys'],text=True,capture_output=True,check=True).stdout;fingerprints=[line.split(':')[9] for line in keys.splitlines() if line.startswith('fpr:')]
 if len(fingerprints)!=1:raise ValueError('Expected exactly one local signing identity')
 fingerprint=fingerprints[0];signature=a.package.with_suffix(a.package.suffix+'.asc')
 subprocess.run(base+['--yes','--armor','--local-user',fingerprint,'--output',str(signature),'--detach-sign',str(a.package)],check=True)
 with a.public_key.open('wb') as output:subprocess.run(base+['--armor','--export',fingerprint],stdout=output,check=True)
 subprocess.run(base+['--verify',str(signature),str(a.package)],check=True)
 print('Local build fingerprint:',fingerprint)
 print('This is a detached local personal-build signature; it does not add an RPM header signature or establish an externally trusted publisher.')
if __name__=='__main__':main()
