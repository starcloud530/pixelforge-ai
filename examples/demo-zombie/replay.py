#!/usr/bin/env python3
"""Rebuild in an empty directory and compare all four native sheet hashes."""
import argparse,hashlib,json,os,pathlib,shutil,subprocess,sys,tempfile

ROOT=pathlib.Path(__file__).resolve().parent
SHEETS=['a/keeper-sheet.png','b/keeper-storm-sheet.png','c/harbor-props-sheet.png','d/iteration-sheet.png']

def main():
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--out',type=pathlib.Path,help='New or empty output directory; defaults to a temporary directory')
 args=parser.parse_args()
 output=args.out.resolve() if args.out else pathlib.Path(tempfile.mkdtemp(prefix='pixelforge-demo-'))
 if output.exists() and any(output.iterdir()):parser.error('--out must be empty; existing assets are never overwritten')
 output.mkdir(parents=True,exist_ok=True)
 env=dict(os.environ,PIXELFORGE_DEMO_OUT=str(output))
 for stage in ['rough','silhouette','flats','detail','directions','walk','outfit','props','local']:
  subprocess.run([sys.executable,str(ROOT/'build.py'),stage],env=env,check=True)
 shutil.copy2(ROOT/'pixelforge.manifest.json',output/'pixelforge.manifest.json')
 subprocess.run([sys.executable,str(ROOT/'validate.py')],env=env,check=True)
 hashes={}
 for name in SHEETS:
  shipped=hashlib.sha256((ROOT/name).read_bytes()).hexdigest()
  rebuilt=hashlib.sha256((output/name).read_bytes()).hexdigest()
  if shipped!=rebuilt:raise RuntimeError('Rebuilt sheet differs: '+name)
  hashes[name]=rebuilt
 report=dict(cleanReplay=True,sheetHashes=hashes,aestheticApproval=False)
 (output/'replay-report.json').write_text(json.dumps(report,indent=2)+'\n')
 print('PASS: all four rebuilt sheets match shipped SHA256 hashes. Output:',output)

if __name__=='__main__':main()
