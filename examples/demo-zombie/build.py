#!/usr/bin/env python3
"""Port keeper case study. All asset pixels are authored by the native kernel.

Python builds command geometry and compares read-only indices. Media assembly
is separate. No image generator, applyPng, IK, or external pixel-buffer writes.
"""
import argparse,collections,json,os,pathlib,shutil,subprocess,time
PACKAGE=pathlib.Path(__file__).resolve().parent
REPO=PACKAGE.parents[1]
ROOT=pathlib.Path(os.environ.get('PIXELFORGE_DEMO_OUT',str(PACKAGE)))
CLI=os.environ.get('PIXELFORGE_BIN') or (str(REPO/'target/debug/pixelforge') if (REPO/'target/debug/pixelforge').exists() else 'pixelforge')
A=ROOT/'a/keeper.pxf.json'
COLORS=['172637','243b50','36536a','587b8c','254e52','3b7776','69a09a','acc8ad',
        '946a50','c69a6c','e8c79a','f5e0b2','a64b37','d77445','efa45c','9c783c',
        'c5a45b','f1d17b','493a32','70533a','a27c4e','798e86','bac4aa','f0e6c3',
        '6d3440','aa5a58','d18b7c','335d62','578a84','f7da7c','fff0bf','0f1b2b']
ops=[];layer=0
def t(tool,**params):ops.append(dict(tool=tool,params=params))
def on(n):
 global layer
 layer=n
def poly(ps,c):t('polygon',points=[dict(x=x,y=y) for x,y in ps],colorIndex=c,fill=True,layer=layer)
def rect(x,y,w,h,c):t('fillRect',x=x,y=y,width=w,height=h,colorIndex=c,layer=layer)
def line(x,y,u,v,c):t('line',x0=x,y0=y,x1=u,y1=v,colorIndex=c,layer=layer)
def ellipse(x,y,w,h,c):t('ellipse',x=x,y=y,width=w,height=h,colorIndex=c,fill=True,layer=layer)
def erase(x,y,w,h):t('eraseRect',x=x,y=y,width=w,height=h,layer=layer)
def cmd(*args):
 start=time.monotonic();p=subprocess.run([CLI,*map(str,args)],capture_output=True,text=True)
 (ROOT/'logs').mkdir(exist_ok=True)
 with (ROOT/'logs/cli.jsonl').open('a') as f:f.write(json.dumps(dict(argv=list(map(str,args)),exit=p.returncode,seconds=round(time.monotonic()-start,4)),ensure_ascii=False)+'\n')
 if p.returncode:raise RuntimeError(p.stdout[-2000:]+p.stderr[-1000:])
 return [json.loads(s) for s in p.stdout.splitlines() if s]
def flush(name,project=A):
 f=ROOT/'logs'/f'{name}.requests.json';f.parent.mkdir(exist_ok=True);f.write_text(json.dumps(ops,indent=2))
 n=len(ops);ops.clear();cmd('call','--project',project,'--request-file',f);print(name,n,'kernel requests')
def inspect(project=A):return cmd('inspect','--project',project)[0]['result']
def snapshot(name,project=A,frame=0,folder='storyboard'):
 d=ROOT/folder;d.mkdir(parents=True,exist_ok=True)
 cmd('preview','--project',project,'--frame',frame,'--scale',6,'--output',d/(name+'.png'))
 shutil.copy2(project,d/(name+'.pxf.json'))
def init(project=A):
 project.parent.mkdir(parents=True,exist_ok=True)
 cmd('new','--width',64,'--height',64,'--output',project)
 for i,h in enumerate(COLORS,1):t('setPaletteColor' if i<=8 else 'addPaletteColor',**({'index':i} if i<=8 else {}),r=int(h[:2],16),g=int(h[2:4],16),b=int(h[4:],16))
 t('renameLayer',index=0,name='body')
 for name in ['legs','arms','gear']:t('addLayer',name=name)
 t('setActiveLayer',index=0);flush('00-init-'+project.stem,project)
def clear():
 for l in range(4):on(l);erase(0,0,64,64)
 on(0)
def body_front(back=False,flat=False):
 on(0)
 poly([(24,28),(37,28),(43,32),(44,43),(41,48),(21,48),(19,43),(20,33)],1)
 poly([(25,30),(36,30),(40,33),(41,44),(38,46),(23,46),(22,42),(23,34)],5)
 poly([(25,32),(30,32),(29,44),(24,45),(23,40)],6)
 poly([(34,31),(38,33),(39,44),(33,45)],2)
 rect(24,43,15,3,19);rect(29,43,5,3,17)
 poly([(24,16),(26,12),(37,12),(40,17),(39,25),(35,29),(27,28),(23,23)],1)
 poly([(26,16),(35,14),(38,17),(37,24),(34,27),(27,25),(25,22)],9 if back else 10)
 if back:
  poly([(25,16),(29,14),(36,15),(38,18),(36,25),(32,28),(26,25),(24,21)],22)
  poly([(25,16),(30,15),(34,17),(34,23),(30,26),(26,23)],23)
 else:
  poly([(26,16),(31,14),(36,16),(36,21),(33,24),(27,22),(25,20)],11)
  rect(23,19,3,5,9);rect(38,19,2,4,9)
  poly([(25,23),(28,24),(31,23),(34,24),(38,22),(38,27),(35,31),(29,32),(25,29)],22)
  poly([(26,25),(29,26),(31,25),(34,26),(36,25),(35,29),(31,30),(28,29)],23)
  line(27,27,29,28,24);line(31,27,31,30,24)
 if not flat:
  if not back:
   line(26,18,29,18,9);line(33,18,36,18,9)
   rect(27,19,2,2,1);rect(34,19,2,2,1);rect(27,19,1,1,12);rect(34,19,1,1,12)
   rect(31,21,2,2,9);rect(31,21,1,1,12)
   line(29,25,33,25,19)
   # Scar and small ear plane, deliberately a different cluster from the beard.
   line(36,21,37,23,13);rect(24,20,1,2,11)
  line(24,34,23,41,7);line(26,34,28,34,8)
  for y in [36,40]:rect(31,y,2,1,17)
  rect(24,38,4,4,5);line(24,38,27,38,7)
  line(36,36,38,42,3);rect(30,44,3,1,18)
  if back:
   line(30,32,29,42,6);line(33,32,35,43,2)
   rect(27,35,8,6,2);line(28,35,33,35,4)
   # Lighthouse patch: a pale vertical stripe and a brass lamp head.
   rect(30,37,2,3,23);rect(29,36,4,1,17)
def legs(facing='down',phase=0):
 on(1)
 # Contacts alternate; the support shoe stays at y=58. This is hand-keyed.
 if facing in ['down','up']:
  poses=[(0,0,0,0),(-3,0,3,-3),(0,-3,0,0),(3,-3,-3,0),(0,0,0,-3)]
  ax,ay,bx,by=poses[phase]
  for x,dx,dy,c in [(23,ax,ay,3),(34,bx,by,2)]:
   poly([(x,44),(x+6,44),(x+6+dx,51+dy),(x+5+dx,55+dy),(x-1+dx,55+dy)],1)
   poly([(x+1,45),(x+4,45),(x+4+dx,51+dy),(x+2+dx,54+dy),(x+dx,52+dy)],c)
   poly([(x-1+dx,53+dy),(x+6+dx,53+dy),(x+7+dx,56+dy),(x+7+dx,58+dy),(x-2+dx,58+dy),(x-2+dx,56+dy)],1)
   rect(x+dx,54+dy,5,3,20);rect(x-1+dx,56+dy,7,2,19)
   line(x+dx,54+dy,x+3+dx,54+dy,21);line(x-1+dx,58+dy,x+6+dx,58+dy,32)
 else:
  poses=[(0,0,0,0),(-5,0,5,-3),(0,-3,0,0),(5,-3,-5,0),(0,0,0,-3)]
  ax,ay,bx,by=poses[phase]
  # Far leg first, near leg second, with genuine overlap in profile.
  for x,dx,dy,c in [(33,bx,by,2),(27,ax,ay,3)]:
   poly([(x,44),(x+5,44),(x+5+dx,50+dy),(x+4+dx,55+dy),(x-1+dx,55+dy),(x-1,50)],1)
   poly([(x+1,45),(x+4,45),(x+3+dx,52+dy),(x+dx,54+dy),(x,49)],c)
   poly([(x-1+dx,53+dy),(x+5+dx,53+dy),(x+5+dx,57+dy),(x+3+dx,58+dy),(x-5+dx,58+dy),(x-5+dx,56+dy)],1)
   rect(x+dx,54+dy,4,3,20);rect(x-4+dx,56+dy,8,2,19)
   line(x-2+dx,56+dy,x+1+dx,56+dy,21)
def arms(facing='down'):
 on(2)
 if facing in ['down','up']:
  poly([(21,31),(25,32),(24,37),(22,43),(20,47),(16,46),(16,40),(18,34)],1)
  poly([(20,33),(23,34),(22,39),(20,43),(17,42),(18,37)],6)
  line(19,35,18,39,7);rect(16,42,5,3,19);rect(17,45,3,3,10);rect(17,45,2,1,11)
  poly([(39,31),(43,32),(46,37),(47,41),(44,44),(40,41),(38,36)],1)
  poly([(41,33),(43,34),(45,38),(44,40),(41,38)],5)
  rect(43,39,4,4,19);rect(45,40,3,3,10);rect(46,40,2,1,11)
 else:
  poly([(34,31),(39,32),(43,36),(43,43),(40,45),(37,41),(35,36)],1)
  poly([(37,33),(39,34),(41,38),(40,41),(38,38)],5)
  rect(39,41,4,3,19);rect(39,44,3,3,9)
  poly([(26,31),(31,31),(32,37),(30,41),(26,44),(22,43),(23,39),(26,37)],1)
  poly([(27,33),(30,33),(30,37),(27,40),(24,40)],6)
  line(27,34,27,37,7);rect(23,40,5,3,19);rect(23,43,4,3,10);rect(23,43,2,1,11)
def lantern(x,y):
 # 10x16 handle, opaque glass and a warm core. No opacity effects.
 on(3)
 line(x+3,y,x+6,y,1);line(x+2,y+1,x+2,y+4,1);line(x+7,y+1,x+7,y+4,1)
 poly([(x+1,y+4),(x+8,y+4),(x+9,y+5),(x+8,y+6),(x+1,y+6),(x,y+5)],1);line(x+2,y+4,x+7,y+4,17)
 poly([(x+1,y+6),(x+8,y+6),(x+9,y+13),(x+7,y+15),(x+2,y+15),(x,y+13)],1)
 rect(x+2,y+6,5,7,28);rect(x+3,y+7,3,6,30);rect(x+4,y+8,2,4,31)
 line(x+1,y+7,x+1,y+12,17);line(x+8,y+7,x+8,y+12,16)
 rect(x+1,y+13,8,2,19);line(x+2,y+14,x+6,y+14,17)
def gear(facing='down',flat=False):
 on(3)
 if facing in ['down','up']:
  poly([(22,13),(22,10),(26,6),(34,5),(40,8),(42,12),(40,16),(24,16)],1)
  poly([(24,12),(25,9),(29,7),(35,7),(39,9),(40,12)],3)
  poly([(25,10),(29,8),(35,8),(37,10),(28,11)],4)
  poly([(23,13),(40,13),(41,14),(38,16),(26,16),(22,15)],2);line(25,14,38,14,4)
  if facing=='down':
   rect(30,11,4,3,17);rect(31,11,2,1,18);rect(31,13,2,1,16)
   poly([(24,28),(29,29),(34,28),(39,29),(38,33),(31,35),(24,33),(22,30)],1)
   poly([(25,29),(29,30),(35,29),(37,30),(35,32),(29,33),(24,31)],14)
   line(25,29,28,30,15);line(31,33,35,32,13)
   poly([(28,33),(32,34),(30,37),(27,39),(25,38),(26,36)],13)
   poly([(28,34),(30,34),(28,37),(26,38),(27,36)],14)
  else:
   line(26,14,37,14,4)
   poly([(24,28),(29,29),(35,28),(39,29),(38,32),(30,34),(23,31)],1)
   poly([(25,29),(29,30),(35,29),(37,30),(34,32),(28,32)],13)
  lantern(46,39)
 else:
  poly([(23,13),(24,9),(29,6),(36,6),(41,9),(42,14),(38,16),(24,16)],1)
  poly([(25,12),(26,9),(31,7),(36,8),(39,10),(40,12)],3)
  poly([(26,10),(31,8),(35,9),(36,10)],4)
  poly([(19,14),(26,12),(40,13),(40,16),(24,17),(19,16)],1)
  line(23,14,38,14,2);rect(26,11,3,2,17)
  poly([(25,28),(31,29),(36,28),(40,30),(37,33),(30,34),(24,31)],1)
  poly([(26,29),(31,30),(36,29),(38,30),(35,32),(29,32)],14)
  poly([(34,33),(38,32),(40,38),(37,41),(35,38)],13)
  poly([(36,34),(37,34),(38,38),(37,39)],14)
  lantern(16,43)
def body_side(flat=False):
 on(0)
 poly([(27,28),(36,28),(41,32),(42,43),(39,48),(25,48),(22,43),(23,35)],1)
 poly([(28,30),(35,30),(38,33),(39,44),(36,46),(26,46),(25,42),(25,35)],5)
 poly([(27,33),(30,32),(29,43),(26,44)],6)
 poly([(36,32),(38,35),(38,44),(34,45)],2)
 rect(25,43,13,3,19);rect(26,43,4,3,17)
 poly([(25,14),(32,12),(38,15),(39,21),(37,27),(32,29),(26,27),(23,24),(22,21),(20,20),(22,18)],1)
 poly([(25,16),(31,14),(35,16),(36,22),(33,25),(27,25),(24,22),(22,21),(24,19)],10)
 poly([(25,16),(29,15),(32,17),(31,21),(27,23),(24,21),(23,20)],11)
 poly([(33,15),(37,16),(38,20),(35,24),(33,22)],22)
 rect(34,18,3,4,9);rect(34,19,2,2,11)
 poly([(23,23),(26,24),(29,23),(33,24),(34,26),(31,30),(27,31),(24,28)],22)
 poly([(24,25),(27,26),(30,25),(31,27),(28,29),(25,28)],23)
 line(25,27,27,28,24);line(28,27,29,29,24);line(26,24,29,24,19)
 if not flat:
  line(24,18,28,18,9);line(24,19,27,19,1);rect(25,19,1,1,12);line(23,20,22,21,9);rect(23,21,1,1,11)
  rect(21,21,2,1,9);line(24,25,27,25,19)
  line(26,34,25,39,7);rect(27,37,2,1,17)
  line(36,35,37,42,3);rect(26,44,2,1,18)
def draw_idle(facing,flat=False):
 (body_side(flat) if facing=='left' else body_front(facing=='up',flat))
 legs(facing);arms(facing);gear(facing,flat)
def sheet(project,path,cols=5,atlas=True,scale=1):
 args=['export','sheet','--project',project,'--output',path,'--cols',cols,'--scale',scale]
 if atlas:args+=['--atlas',path.with_suffix('.atlas.json')]
 cmd(*args)
def stage_rough():
 init();on(0)
 rect(24,8,15,18,1);rect(20,27,23,22,1);rect(21,48,8,11,1);rect(35,48,8,11,1)
 rect(16,30,5,16,1);rect(43,30,5,16,1);rect(46,45,8,10,1)
 flush('01-rough');snapshot('v0-rough')
def stage_silhouette():
 clear()
 # One-colour silhouette, no facial detail or material colours.
 for ps in [[(22,13),(23,9),(29,5),(36,6),(41,10),(41,17),(39,25),(35,30),(26,29),(23,24)],[(24,28),(38,28),(44,34),(44,44),(40,48),(22,48),(19,43),(20,33)],[(21,31),(25,32),(23,40),(20,48),(16,47),(16,40)],[(39,31),(43,32),(47,39),(48,43),(44,44),(40,40)],[(23,45),(29,45),(29,55),(30,58),(21,58),(21,55)],[(34,45),(40,45),(40,55),(42,58),(33,58)],[(47,42),(53,42),(55,52),(52,54),(47,54),(45,51)]]:poly(ps,1)
 flush('02-silhouette');snapshot('v1-silhouette')
def stage_flats():
 clear();draw_idle('down',True);flush('03-flats');snapshot('v2-flats')
def stage_detail():
 # Touch up the existing body only; don't redraw unchanged limbs/equipment.
 on(0);line(26,18,29,18,9);line(33,18,36,18,9)
 rect(27,19,2,2,1);rect(34,19,2,2,1);rect(27,19,1,1,12);rect(34,19,1,1,12)
 rect(31,21,2,2,9);rect(31,21,1,1,12);line(29,25,33,25,19)
 line(36,21,37,23,13);rect(24,20,1,2,11)
 line(24,34,23,41,7);line(26,34,28,34,8)
 for y in [36,40]:rect(31,y,2,1,17)
 rect(24,38,4,4,5);line(24,38,27,38,7);line(36,36,38,42,3);rect(30,44,3,1,18)
 t('setPaletteLocked',locked=True);flush('04-detail');snapshot('v3-details')
def stage_directions():
 # Allocate a fixed5-column sheet, then copy same-pose pixels into its slots.
 for _ in range(19):t('addFrame')
 for frame,facing in [(5,'left'),(15,'up')]:
  t('setActiveFrame',index=frame);draw_idle(facing)
 # Genuine side profile is mirrored, never rotated, then relit.
 for l in range(4):t('copyRect',fromFrame=5,toFrame=10,srcX=0,srcY=0,width=64,height=64,dstX=0,dstY=0,layer=l)
 t('setActiveFrame',index=10)
 for l in range(4):t('flipHorizontal',layer=l)
 on(0);poly([(26,33),(28,32),(28,43),(26,44)],6);line(26,34,26,39,7)
 on(3);line(26,9,30,8,4);line(36,10,38,12,2)
 # Scar belongs to the character's left cheek: hidden in left profile, visible in right.
 on(0);line(36,22,37,24,13);rect(36,23,1,1,27)
 flush('05-directions');snapshot('v4-directions',frame=0)
 sheet(A,ROOT/'storyboard/v4-four-idles.png',cols=5,atlas=False,scale=3)
def stage_walk():
 for row,facing in enumerate(['down','left','right','up']):
  source=row*5
  for phase in range(1,5):
   f=source+phase
   for l in range(4):t('copyRect',fromFrame=source,toFrame=f,srcX=0,srcY=0,width=64,height=64,dstX=0,dstY=0,layer=l)
   t('setActiveFrame',index=f)
   bob=-1 if phase in [2,4] else 0
   if bob:
    for l in [0,3]:t('moveRect',srcX=0,srcY=1,width=64,height=62,dstX=0,dstY=0,layer=l)
   on(1);erase(0,43,64,21)
   if facing=='right':
    legs('left',phase);t('flipHorizontal',layer=1)
   else:legs(facing,phase)
   swing=1 if phase in [1,2] else -1
   on(2)
   if facing in ['down','up']:
    t('moveRect',srcX=15,srcY=30,width=11,height=20,dstX=15+swing,dstY=30+bob,layer=2)
    if bob:t('moveRect',srcX=38,srcY=30,width=11,height=17,dstX=38,dstY=29,layer=2)
   else:
    # Near and far arms share the body bob; their elbows remain connected.
    # Keep the lantern-bearing hand aligned with its handle. This arm is
    # carrying a rigid prop rather than swinging freely.
    t('moveRect',srcX=15,srcY=29,width=35,height=21,dstX=15,dstY=29+bob,layer=2)
   t('setFrameDelay',index=f,delayMs=140 if phase in [1,3] else 110)
 for i in [0,5,10,15]:t('setFrameDelay',index=i,delayMs=400)
 t('setDirectionLayout',count=4,columns=5,apply=True)
 flush('06-walk');snapshot('v5-walk',frame=1)
 sheet(A,ROOT/'a/keeper-sheet.png');sheet(A,ROOT/'a/keeper-sheet-preview.png',scale=3,atlas=False)
 cmd('export','png','--project',A,'--out-dir',ROOT/'a/frames','--scale',1)
 for facing in ['down','left','right','up']:cmd('export','apng','--project',A,'--tag','walk_'+facing,'--output',ROOT/f'a/walk-{facing}.apng','--scale',4)
 cmd('direction','validate','--project',A)
 cmd('validate-atlas','--atlas',ROOT/'a/keeper-sheet.atlas.json','--sheet',ROOT/'a/keeper-sheet.png','--strict')
def cape(facing,bob=0):
 on(4)
 def p(ps,c):poly([(63-x if facing=='right' else x,y+bob) for x,y in ps],c)
 def ln(x,y,u,v,c):line(63-x if facing=='right' else x,y+bob,63-u if facing=='right' else u,v+bob,c)
 if facing in ['down']:
  p([(22,29),(26,31),(25,35),(22,39),(18,39),(18,35)],1)
  p([(22,31),(24,32),(22,36),(20,37),(20,35)],6)
  p([(36,29),(41,30),(44,34),(46,43),(45,50),(40,51),(38,43),(38,36),(34,33)],1)
  p([(38,31),(41,32),(42,36),(44,44),(43,48),(41,48),(40,42),(40,35)],6)
  p([(38,32),(40,33),(41,38),(40,39),(38,35)],7)
  ln(41,47,44,48,17)
  ln(20,37,23,36,17)
 elif facing=='up':
  p([(23,28),(28,30),(36,28),(40,30),(44,40),(46,49),(40,52),(34,49),(28,52),(20,50),(19,42)],1)
  p([(24,30),(29,32),(35,30),(39,32),(41,40),(43,48),(40,49),(34,46),(28,49),(22,48),(22,42)],6)
  p([(24,32),(27,33),(26,43),(23,47),(23,40)],7)
  p([(35,33),(38,33),(40,44),(39,47),(36,44)],5)
  ln(23,48,28,50,17);ln(28,50,34,47,17);ln(34,47,40,50,17);ln(40,50,43,49,17)
  p([(30,36),(33,39),(31,43),(28,40)],17)
  ln(30,38,31,41,18)
 else:
  p([(34,28),(39,29),(43,34),(44,42),(47,49),(42,52),(35,48),(34,39),(31,33)],1)
  p([(35,30),(38,31),(40,35),(41,43),(44,48),(41,49),(37,46),(36,38),(33,33)],6)
  p([(35,31),(37,32),(38,37),(37,40),(35,36)],7)
  ln(38,47,42,50,17);ln(42,50,44,49,17)
def stage_outfit():
 b=ROOT/'b/keeper-storm-cape.pxf.json';b.parent.mkdir(exist_ok=True);shutil.copy2(A,b)
 t('setActiveLayer',index=3);t('addLayer',name='storm-cape')
 for frame in range(20):
  t('setActiveFrame',index=frame);cape(['down','left','right','up'][frame//5],-1 if frame%5 in [2,4] else 0)
 flush('07-outfit',b)
 sheet(b,ROOT/'b/keeper-storm-sheet.png');sheet(b,ROOT/'b/keeper-storm-sheet-preview.png',atlas=False,scale=3)
 cmd('preview','--project',b,'--frame',0,'--scale',6,'--output',ROOT/'b/after.png')
 cmd('preview','--project',A,'--frame',0,'--scale',6,'--output',ROOT/'b/before.png')
 cmd('export','png','--project',b,'--out-dir',ROOT/'b/frames','--scale',1)
 cmd('validate-atlas','--atlas',ROOT/'b/keeper-storm-sheet.atlas.json','--strict')
 snapshot('v6-outfit',b,0)
 a=json.loads(A.read_text());bd=json.loads(b.read_text())
 unchanged=all(x['layers'][:4]==y['layers'][:4] for x,y in zip(a['frames'],bd['frames']))
 assert unchanged and a['palette']==bd['palette']
 added=[sum(v!=0 for v in f['layers'][4]['pixels']) for f in bd['frames']]
 (ROOT/'b/verification.json').write_text(json.dumps(dict(baseLayersExactlyUnchanged=unchanged,paletteExactlyUnchanged=True,newLayer='storm-cape',newInkPerFrame=added),indent=2))
 print('B base layers unchanged; added cells',added)
def prop_bag():
 on(0)
 poly([(17,24),(24,20),(40,20),(47,25),(48,46),(43,51),(21,51),(16,46)],1)
 poly([(19,26),(25,23),(39,23),(44,27),(45,45),(41,48),(23,48),(19,44)],5)
 poly([(20,27),(27,25),(27,45),(23,46),(21,42)],6)
 poly([(30,25),(39,25),(42,28),(42,43),(38,46),(30,46)],2)
 poly([(18,25),(23,20),(40,20),(46,25),(43,32),(21,32)],1)
 poly([(21,25),(25,22),(38,22),(42,25),(41,29),(23,29)],6)
 line(23,25,27,24,7)
 rect(25,30,4,16,19);rect(36,30,4,16,19)
 rect(24,34,6,5,17);rect(25,35,4,3,19);rect(35,34,6,5,17);rect(36,35,4,3,19)
 line(26,44,28,44,21);line(37,44,39,44,21)
 line(25,20,25,16,1);line(25,16,37,16,1);line(37,16,39,20,1)
 line(27,18,35,18,20)
def prop_lamp():
 on(0)
 line(26,12,37,12,1);line(24,14,24,22,1);line(39,14,39,22,1)
 line(26,14,36,14,17);line(25,15,25,20,16)
 poly([(23,22),(40,22),(43,26),(42,29),(21,29),(20,26)],1)
 rect(23,24,16,3,20);line(24,24,36,24,17)
 poly([(23,29),(40,29),(43,45),(40,50),(23,50),(20,45)],1)
 poly([(25,30),(37,30),(39,43),(36,47),(26,47),(23,43)],28)
 poly([(27,32),(35,32),(36,42),(34,45),(27,44),(26,39)],29)
 poly([(30,33),(33,34),(35,39),(33,43),(29,42),(28,38)],30)
 rect(30,36,3,5,31)
 line(23,31,22,44,17);line(40,31,41,44,16)
 rect(22,47,19,4,19);line(24,48,37,48,17);line(24,51,38,51,1)
def prop_crate():
 on(0)
 poly([(15,23),(35,17),(49,24),(49,46),(29,53),(15,46)],1)
 poly([(17,25),(29,30),(29,50),(17,44)],20)
 poly([(30,31),(47,26),(47,44),(30,50)],19)
 poly([(18,23),(35,19),(45,24),(29,28)],21)
 for y in [29,35,41]:line(18,y,27,y+3,19)
 for y in [32,38,44]:line(32,y,45,y-4,20)
 poly([(18,25),(21,26),(27,46),(26,48),(24,47)],21)
 poly([(32,32),(35,31),(45,41),(45,44),(43,44)],20)
 for x,y in [(18,26),(26,29),(18,43),(26,47),(32,32),(45,28),(32,47),(45,43)]:rect(x,y,1,2,17)
def prop_buoy():
 on(0)
 rect(29,10,3,9,1);rect(31,11,1,7,17)
 poly([(32,10),(42,13),(32,16)],13);line(33,11,38,13,15)
 ellipse(19,43,29,10,1);ellipse(21,44,25,7,28);line(23,48,42,48,29)
 poly([(27,21),(36,21),(41,36),(44,43),(39,48),(25,48),(21,43),(24,35)],1)
 poly([(28,23),(34,23),(38,35),(40,41),(37,45),(27,45),(24,41),(27,33)],14)
 poly([(28,24),(30,24),(29,35),(27,39),(26,41),(25,39)],15)
 poly([(26,32),(36,32),(38,37),(25,37)],23)
 poly([(32,24),(34,24),(37,32),(34,32)],13)
 line(25,42,38,42,13);rect(29,17,5,5,1);rect(30,18,3,2,17)
def stage_props():
 p=ROOT/'c/harbor-props.pxf.json';init(p)
 for _ in range(3):t('addFrame')
 for i,(name,fn) in enumerate([('bag',prop_bag),('lantern',prop_lamp),('crate',prop_crate),('buoy',prop_buoy)]):
  t('setActiveFrame',index=i);start=len(ops);fn()
  def grow(v):return max(1,min(62,round(32+(v-32)*1.12)))
  for request in ops[start:]:
   q=request['params']
   if 'points' in q:q['points']=[dict(x=grow(a['x']),y=grow(a['y'])) for a in q['points']]
   if 'x0' in q:
    for key in ['x0','y0','x1','y1']:q[key]=grow(q[key])
   if 'x' in q:
    x,y=q['x'],q['y'];q['x'],q['y']=grow(x),grow(y)
    if 'width' in q:q['width']=max(1,grow(x+q['width'])-grow(x))
    if 'height' in q:q['height']=max(1,grow(y+q['height'])-grow(y))
  t('addTag',name=name,**{'from':i,'to':i,'loop':'once'})
 t('setPaletteLocked',locked=True);flush('08-props',p)
 sheet(p,ROOT/'c/harbor-props-sheet.png',cols=4);sheet(p,ROOT/'c/harbor-props-preview.png',cols=4,scale=4,atlas=False)
 cmd('export','png','--project',p,'--out-dir',ROOT/'c/frames','--scale',1)
 cmd('validate-atlas','--atlas',ROOT/'c/harbor-props-sheet.atlas.json','--four-direction','off','--strict')
 assert json.loads(p.read_text())['palette']==json.loads(A.read_text())['palette']
def shield():
 on(4)
 poly([(12,34),(18,31),(25,34),(25,44),(22,49),(18,52),(14,48),(11,43)],1)
 poly([(14,35),(18,33),(23,35),(23,43),(21,47),(18,49),(15,46),(13,42)],17)
 poly([(15,36),(18,35),(21,36),(21,43),(19,46),(18,47),(16,44),(15,41)],5)
 line(18,36,18,46,7);line(15,39,21,39,6)
 rect(17,39,3,3,18);rect(18,40,1,1,16)
def stage_local():
 d=ROOT/'d';d.mkdir(exist_ok=True);p=d/'local-iterations.pxf.json';shutil.copy2(A,p)
 # Keep one walk contact pose. Subsequent timeline frames are duplicates,
 # enabling real native diffFrames (not independently regenerated images).
 t('clearDirectionLayout')
 for tag in json.loads(A.read_text())['tags']:t('removeTag',name=tag['name'])
 for i in range(19,1,-1):t('deleteFrame',index=i)
 t('deleteFrame',index=0);t('setActiveFrame',index=0)
 # Explicit diagnostic fixture: move only the planted boot +2 px.
 t('moveRect',srcX=18,srcY=53,width=10,height=6,dstX=20,dstY=53,layer=1)
 flush('09-diagnostic-fixture',p);snapshot('00-before',p,0,'d')
 t('duplicateFrame',index=0);t('setActiveFrame',index=1)
 t('setSelection',x=21,y=28,width=20,height=14)
 for old,new in [(13,25),(14,26),(15,27)]:t('replaceIndex',fromIndex=old,toIndex=new,layer=3)
 t('clearSelection');flush('10-local-recolor',p);snapshot('01-recolor',p,1,'d')
 t('duplicateFrame',index=1);t('setActiveFrame',index=2)
 t('setActiveLayer',index=3);t('addLayer',name='shield');shield()
 flush('11-local-shield',p);snapshot('02-shield',p,2,'d')
 t('duplicateFrame',index=2);t('setActiveFrame',index=3)
 t('moveRect',srcX=20,srcY=53,width=10,height=6,dstX=18,dstY=53,layer=1)
 for i in range(4):t('addTag',name=['before','recolor','shield','foot-fix'][i],**{'from':i,'to':i,'loop':'once'})
 flush('12-local-foot-fix',p);snapshot('03-foot-fix',p,3,'d')
 reports=[]
 for i in range(3):
  r=cmd('diff','--project',p,'--frame',i,'--with',i+1,'--scale',4,'--output',d/f'{i+1:02}-diff.png')[0]
  (d/f'{i+1:02}-diff.json').write_text(json.dumps(r,indent=2));reports.append(r['result'])
 sheet(p,d/'iteration-sheet.png',cols=4)
 cmd('validate-atlas','--atlas',d/'iteration-sheet.atlas.json','--four-direction','off','--strict')
 cmd('export','png','--project',p,'--out-dir',d/'frames','--scale',1)
 frames=json.loads(p.read_text())['frames']
 # Full body layer identical, not merely visually similar.
 assert all(f['layers'][0]==frames[0]['layers'][0] for f in frames)
 assert frames[0]['layers'][1]==frames[1]['layers'][1]==frames[2]['layers'][1]
 assert frames[1]['layers'][:4]==frames[2]['layers'][:4]
 assert frames[2]['layers'][0]==frames[3]['layers'][0]
 a=json.loads(A.read_text())['frames'][1]
 assert frames[3]['layers'][1]==a['layers'][1]
 (d/'verification.json').write_text(json.dumps(dict(bodyIdenticalAcrossAllSteps=True,shieldStepBaseLayersUnchanged=True,correctedLegsEqualOriginalAContact=True,fixture='Deliberately shifted support boot +2px; not a historical spontaneous defect.',changedPixels=[r['changed'] for r in reports]),indent=2))
 print('D changed composite pixels:',[r['changed'] for r in reports])
def main():
 parser=argparse.ArgumentParser();parser.add_argument('stage',choices=['rough','silhouette','flats','detail','directions','walk','outfit','props','local']);args=parser.parse_args()
 ROOT.mkdir(exist_ok=True);(ROOT/'logs').mkdir(exist_ok=True)
 globals()['stage_'+args.stage]()
if __name__=='__main__':main()
