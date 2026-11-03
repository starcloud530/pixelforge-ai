#!/usr/bin/env python3
"""Validate shipped demo artifacts; reports structure and identity, not taste."""
import json,pathlib,subprocess,os
ROOT=pathlib.Path(os.environ.get('PIXELFORGE_DEMO_OUT',str(pathlib.Path(__file__).resolve().parent)))
REPO=pathlib.Path(__file__).resolve().parents[2]
CLI=os.environ.get('PIXELFORGE_BIN') or (str(REPO/'target/debug/pixelforge') if (REPO/'target/debug/pixelforge').exists() else 'pixelforge')
def load(p):return json.loads((ROOT/p).read_text())
def run(*args):
 r=subprocess.run([CLI,*map(str,args)],capture_output=True,text=True)
 if r.returncode:raise RuntimeError(r.stdout+r.stderr)
 return json.loads(r.stdout)
def indices(f):
 return [next((l['pixels'][i] for l in reversed(f['layers']) if l['pixels'][i]),0) for i in range(4096)]
a=load('a/keeper.pxf.json');b=load('b/keeper-storm-cape.pxf.json');c=load('c/harbor-props.pxf.json');d=load('d/local-iterations.pxf.json')
assert a['width']==a['height']==64 and len(a['frames'])==20
assert len(a['palette'])==33 and a['palette']==b['palette']==c['palette']==d['palette']
for p in ['a/keeper.pxf.json','b/keeper-storm-cape.pxf.json']:
 run('direction','validate','--project',ROOT/p)
run('project','validate','--manifest',ROOT/'pixelforge.manifest.json')
run('asset','validate','--manifest',ROOT/'pixelforge.manifest.json')
reports=[]
for atlas,kind in [('a/keeper-sheet.atlas.json','on'),('b/keeper-storm-sheet.atlas.json','on'),('c/harbor-props-sheet.atlas.json','off'),('d/iteration-sheet.atlas.json','off')]:
 reports.append(run('validate-atlas','--atlas',ROOT/atlas,'--four-direction',kind,'--strict')['result'])
assert all(fa['layers']==fb['layers'][:4] for fa,fb in zip(a['frames'],b['frames']))
assert len(c['frames'])==4 and {t['name'] for t in c['tags']}=={'bag','lantern','crate','buoy'}
assert all(f['layers'][0]==d['frames'][0]['layers'][0] for f in d['frames'])
assert d['frames'][1]['layers'][:4]==d['frames'][2]['layers'][:4]
assert d['frames'][3]['layers'][1]==a['frames'][1]['layers'][1]
ground=[];unique_walk=[]
for row in range(4):
 walks=[indices(a['frames'][row*5+i]) for i in range(1,5)]
 assert len({tuple(x) for x in walks})==4
 unique_walk.append(4)
for f in a['frames']:
 ink=[i for i,v in enumerate(indices(f)) if v];ground.append(max(ink)//64)
assert all(y==58 for y in ground)
for i in range(3):
 before=indices(d['frames'][i]);after=indices(d['frames'][i+1])
 assert all(before[y*64+x]==after[y*64+x] for y in range(28) for x in range(64))
report=dict(atlasChecks=reports,directionsValid=True,baseLayersUnchangedOnOutfit=True,paletteShared=True,uniqueWalkPosesPerDirection=unique_walk,groundRows=ground,faceUnchangedAcrossLocalEdits=True,footFixExactlyRestoresOriginal=True,aestheticApproval=False)
(ROOT/'validation.json').write_text(json.dumps(report,indent=2));print('PASS:4atlases,4directions,20ground rows, outfit identity, shared palette, local edits. Aesthetics not certified.')
