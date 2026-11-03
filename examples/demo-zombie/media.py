#!/usr/bin/env python3
"""Presentation only: native PNGs are composited without repainting asset pixels.

MP4s are captioned CLI-result walkthroughs, not recordings of a GUI or invented
live terminal session. All command transcripts and native diffs accompany them.
"""
import collections,json,pathlib,subprocess,sys,time,shutil,os
ROOT=pathlib.Path(os.environ.get('PIXELFORGE_DEMO_OUT',str(pathlib.Path(__file__).resolve().parent)))
from PIL import Image,ImageDraw,ImageFont
import imageio_ffmpeg
BG='#12242d';PANEL='#1d3540';TEXT='#f1e5c6';MUTED='#98b6b5';ORANGE='#efa45c'
FONT='/System/Library/Fonts/Supplemental/Arial.ttf'
MONO='/System/Library/Fonts/Menlo.ttc'
def font(n,mono=False):
 try:return ImageFont.truetype(MONO if mono else FONT,n)
 except OSError:return ImageFont.load_default(size=n)
def canvas(w,h):return Image.new('RGB',(w,h),BG)
def label(im,xy,text,n=24,color=TEXT,mono=False):ImageDraw.Draw(im).text(xy,text,font=font(n,mono),fill=color)
def image(path):return Image.open(ROOT/path).convert('RGBA')
def paste(im,sprite,xy,size):
 s=sprite.resize(size,Image.Resampling.NEAREST);im.paste(s,xy,s)
def panel(im,box):ImageDraw.Draw(im).rounded_rectangle(box,radius=18,fill=PANEL)
def frame(row,col=0,variant='a'):
 return image(f'{variant}/frames/pixelforge-{row*5+col:03}.png')
def save(im,path):
 p=ROOT/path;p.parent.mkdir(parents=True,exist_ok=True);im.save(p)
def gif():
 seq=[]
 for phase in range(1,5):
  im=canvas(1024,352)
  label(im,(24,12),'PORT KEEPER / FOUR KEYED WALK CYCLES',24)
  for row,name in enumerate(['SOUTH','WEST','EAST','NORTH']):
   paste(im,frame(row,phase),(row*256,48),(256,256));label(im,(row*256+90,312),name,18,MUTED)
  seq.append(im)
 seq[0].save(ROOT/'a/walk-four-directions.gif',save_all=True,append_images=seq[1:],duration=[140,110,140,110],loop=0,disposal=2)
def stills():
 hero=canvas(1280,720)
 label(hero,(50,28),'PIXELFORGE / PORT KEEPER',34)
 label(hero,(50,76),'Local CLI edits. Indexed pixels. Inspectable exports.',22,MUTED)
 panel(hero,(40,125,610,645));panel(hero,(635,125,1240,645))
 paste(hero,image('storyboard/v0-rough.png'),(165,200),(320,320))
 paste(hero,frame(0),(780,200),(320,320))
 label(hero,(65,603),'BEFORE / deliberately rough blockout',22,MUTED)
 label(hero,(660,603),'AFTER / native64x64 +32colors',22,ORANGE)
 label(hero,(50,676),'A worked example, not a promise of unattended generation or aesthetic validation.',19,MUTED)
 save(hero,'storyboard/hero-before-after.png')
 outfit=canvas(960,580)
 label(outfit,(30,20),'B / SAME KEEPER, NEW STORM CAPE',28)
 paste(outfit,frame(0),(50,70),(384,384));paste(outfit,frame(0,variant='b'),(520,70),(384,384))
 label(outfit,(90,474),'BASE OUTFIT',22,MUTED);label(outfit,(540,474),'GEAR LAYER ADDED',22,ORANGE)
 label(outfit,(30,528),'Body / legs / arms / original gear: byte-for-byte unchanged.',22)
 save(outfit,'b/before-after.png')
 steps=['00-before','01-recolor','02-shield','03-foot-fix']
 d=canvas(1280,430);label(d,(28,15),'D / LOCAL ITERATIONS, NOT FULL-IMAGE REGENERATION',28)
 for i,(s,title) in enumerate(zip(steps,['0 / FOOT-DRIFT FIXTURE','1 / SCARF RECOLOR','2 / SHIELD LAYER','3 / BOOT -2PX'])):
  paste(d,image('d/'+s+'.png'),(i*320+16,65),(288,288));label(d,(i*320+20,368),title,18,ORANGE if i else MUTED)
 label(d,(28,404),'The initial boot drift was deliberately introduced for this reproducible diagnostic.',16,MUTED)
 save(d,'d/local-iterations.png')
 # Seven actual production stages. The fourth view and sheet use native exports.
 storyboard=canvas(1400,820)
 label(storyboard,(30,18),'A REAL COMMAND HISTORY /7 STAGES',30)
 stages=[('v0-rough.png','0 / rough blockout'),('v1-silhouette.png','1 / repair silhouette'),('v2-flats.png','2 / separate materials'),('v3-details.png','3 / face + seam detail')]
 for i,(f,title) in enumerate(stages):
  paste(storyboard,image('storyboard/'+f),(i*345+12,70),(320,320));label(storyboard,(i*345+20,395),title,21,MUTED)
 for i,title in enumerate(['4 / redraw profile + rear','5 / copy pose, edit limbs','6 / add storm-cape layer']):
  x=i*460+30
  pic=frame(1) if i==0 else (frame(0,1) if i==1 else frame(0,variant='b'))
  paste(storyboard,pic,(x+45,445),(280,280));label(storyboard,(x,747),title,21,ORANGE)
 save(storyboard,'storyboard/timeline.png')
def chapter(title,subtitle,commands):
 im=canvas(1280,720);label(im,(40,26),'PIXELFORGE / CLI WORKFLOW',22,ORANGE)
 label(im,(40,70),title,32);label(im,(40,117),subtitle,20,MUTED)
 panel(im,(28,158,790,650));panel(im,(808,158,1252,650))
 label(im,(830,180),'ACTUAL COMMANDS',18,ORANGE)
 y=222
 for line in commands:
  label(im,(830,y),line,15,TEXT,True);y+=25
 label(im,(40,680),'Captioned walkthrough of real CLI artifacts / not a GUI recording / no generated pixels',17,MUTED)
 return im
def video1(k):
 # 40 seconds, five8-second chapters, actual exported states.
 if k<8:
  im=chapter('01 / INSTALL + CHECK','Local source-build binary installed for this demonstration.',
   ['install -m755', '  target/debug/pixelforge','  /tmp/pf-demo-bin/pixelforge','','pixelforge --version','','pixelforge new','  --width 64 --height 64','  --output keeper.pxf.json'])
  paste(im,image('storyboard/v0-rough.png'),(175,194),(410,410))
 elif k<16:
  im=chapter('02 / COMMANDS TO AN IDLE','Silhouette, material blocks, then face and clothing detail.',
   ['pixelforge call','  --project keeper.pxf.json','  --request-file stage.json','','# polygon / line / fillRect','','pixelforge preview','  --project keeper.pxf.json','  --scale 6'])
  paste(im,image('storyboard/'+['v1-silhouette.png','v2-flats.png','v3-details.png'][min(2,int((k-8)/2.67))]),(175,194),(410,410))
 elif k<24:
  im=chapter('03 / ONE IDENTITY, FOUR VIEWS','A redrawn profile and rear. Opposite profile mirrored and relit.',
   ['# row order: S W E N','# each row: idle +4walk','','pixelforge call','  --request-file views.json','','# no sprite rotations','# no automatic IK gait'])
  for row in range(4):paste(im,frame(row),(48+row*180,285),(180,180))
 elif k<32:
  im=chapter('04 / KEYED WALK + TAGS','Four manually keyed poses per facing, with110/140ms timing.',
   ['# native copyRect + moveRect','# limb edits on copied pose','','pixelforge direction set','  --count 4 --columns 5','  --apply','','idle_down /walk_down','idle_left /walk_left','idle_right/walk_right','idle_up   /walk_up'])
  phase=1+int((k-24)*8)%4
  for row in range(4):paste(im,frame(row,phase),(48+row*180,285),(180,180))
 else:
  im=chapter('05 / EXPORT + VALIDATE','Structural checks pass. This is not an aesthetic certification.',
   ['pixelforge export sheet','  --cols 5 --scale 1','  --output keeper-sheet.png','  --atlas keeper.atlas.json','','pixelforge validate-atlas','  --atlas keeper.atlas.json','  --strict','','0 errors /0 warnings'])
  paste(im,image('a/keeper-sheet.png'),(118,173),(600,480))
 return im
def video2(k):
 # 60 seconds, five12-second chapters.
 if k<12:
  im=chapter('01 / OPEN THE INDEXED PROJECT','Four base layers.32palette colors. No model call.',
   ['pixelforge scene','  --project keeper.pxf.json','','body /legs /arms /gear','','pixelforge preview','  --scale 6','','# source: copied walk pose'])
  paste(im,image('d/00-before.png'),(175,194),(410,410))
 elif k<24:
  im=chapter('02 / RECOLOR ONLY THE SCARF','Mask the scarf. Replace three indices on the gear layer.',
   ['pixelforge selection set','  --x 21 --y 28','  --width 20 --height 14','','# gear only:','13 ->25 /14 ->26 /15 ->27','','pixelforge diff','  --frame 0 --with 1'])
  paste(im,image('d/00-before.png'),(60,235),(320,320));paste(im,image('d/01-recolor.png'),(430,235),(320,320))
 elif k<36:
  im=chapter('03 / ADD A SHIELD LAYER','The existing body and equipment layers do not change.',
   ['pixelforge layer add','  --name shield','','pixelforge call','  --request-file shield.json','','# polygons + lines only','','pixelforge diff','  --frame 1 --with 2'])
  paste(im,image('d/01-recolor.png'),(60,235),(320,320));paste(im,image('d/02-shield.png'),(430,235),(320,320))
 elif k<48:
  im=chapter('04 / FIX THE PLANTED BOOT','Deliberate+2px diagnostic drift. Move only the boot back.',
   ['# fixture, not a historical bug','pixelforge move-rect','  --layer 1','  --src-x 20 --src-y 53','  --width 10 --height 6','  --dst-x 18 --dst-y 53','','pixelforge diff','  --frame 2 --with 3'])
  paste(im,image('d/02-shield.png'),(60,205),(320,320));paste(im,image('d/03-foot-fix.png'),(430,205),(320,320))
  for i in [2,3]:
   foot=image(f'd/frames/pixelforge-{i:03}.png').crop((16,50,44,60))
   paste(im,foot,(110+(i-2)*350,560),(224,80))
 else:
  im=chapter('05 / RE-EXPORT THE EDIT HISTORY','Each step keeps before/after PNGs, native diffs and a project.',
   ['pixelforge export sheet','  --project local.pxf.json','  --cols 4','  --atlas iteration.atlas.json','','pixelforge validate-atlas','  --four-direction off','','# body layer unchanged','# original foot restored'])
  paste(im,image('d/iteration-sheet.png'),(40,276),(736,184))
 return im
def videos():
 ff=imageio_ffmpeg.get_ffmpeg_exe();(ROOT/'videos').mkdir(exist_ok=True)
 for name,seconds,fn in [('01-sheet-assembly.mp4',40,video1),('02-local-editing.mp4',60,video2)]:
  p=subprocess.Popen([ff,'-y','-loglevel','error','-f','rawvideo','-pix_fmt','rgb24','-s','1280x720','-r','12','-i','-','-an','-c:v','libx264','-preset','fast','-crf','19','-pix_fmt','yuv420p','-movflags','+faststart',str(ROOT/'videos'/name)],stdin=subprocess.PIPE)
  try:
   for i in range(seconds*12):p.stdin.write(fn(i/12).tobytes())
   p.stdin.close();assert p.wait()==0
  except BaseException:
   p.kill();raise
  print(name,seconds,'seconds')
  fn(5).save(ROOT/'videos'/name.replace('.mp4','-poster.png'))
def main():
 if len(sys.argv)>1 and sys.argv[1]=='videos':videos()
 else:gif();stills()
if __name__=='__main__':main()
