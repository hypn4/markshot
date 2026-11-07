import json, sys, math, subprocess, os
from PIL import Image
D=[22,28,36,48,64,84,112,150,200,266]
EVAL_DIR=os.environ['EVAL_DIR']; BIN=f'{EVAL_DIR}/bin/markshot'; SHOT=f'{EVAL_DIR}/shot.png'
def contains(r,b,m=0):  # r=[x,y,w,h], b=(x0,y0,x1,y1)
    return r[0]<=b[0]-m and r[1]<=b[1]-m and r[0]+r[2]>=b[2]+m and r[1]+r[3]>=b[3]+m
def dist_pt_box(p,b):
    dx=max(b[0]-p[0],0,p[0]-b[2]); dy=max(b[1]-p[1],0,p[1]-b[3]); return math.hypot(dx,dy)
def tip(c,dpr):
    R=D[c['size']]/2*dpr; a=math.radians(c['angle'])
    if c['style']=='circle': return c['center']
    return [c['center'][0]+1.65*R*math.cos(a), c['center'][1]+1.65*R*math.sin(a)]
def load(run):
    rep=json.load(open(f'{run}/report.json')); return rep, rep['spec']['annotations'], rep['spec']['dpr']
def rerender_matches(run):
    subprocess.run([BIN,'render',SHOT,'--spec','-','-o',f'{run}/_re.png'],input=json.dumps(json.load(open(f'{run}/report.json'))['spec']).encode(),capture_output=True,check=True)
    return open(f'{run}/_re.png','rb').read()==open(f'{run}/out.png','rb').read()
def s1(run):
    rep,A,dpr=load(run); res={}
    blue=(1114,279,1266,306); green=(1375,80,1519,105); yellow=(1373,279,1533,306)
    blurs=[a for a in A if a['type']=='blur']; rects=[a for a in A if a['type'] in('rect','oval')]; cs=sorted([a for a in A if a['type']=='counter'],key=lambda a:a['number'])
    res['blur_covers_secret']=any(contains(b['rect'],blue) for b in blurs)
    res['box_around_erase']=any(contains(r['rect'],green) and r['rect'][2]<400 and r['rect'][3]<200 for r in rects)
    res['counter1_near_erase']=len(cs)>=1 and cs[0]['number']==1 and min(dist_pt_box(tip(cs[0],dpr),green),dist_pt_box(cs[0]['center'],green))<140
    res['counter2_near_text']=len(cs)>=2 and cs[1]['number']==2 and min(dist_pt_box(tip(cs[1],dpr),yellow),dist_pt_box(cs[1]['center'],yellow))<140
    res['no_warnings']=rep['warnings']==[]
    res['out_from_original']=rerender_matches(run)
    return res
def s2(run):
    rep,A,dpr=load(run); res={}
    r=[a for a in A if a['type']=='rect']; c=[a for a in A if a['type']=='counter']
    im=Image.open(f'{run}/out.png').convert('RGB'); p=im.getpixel((1180,495))
    res['rect_opaque_dark']=bool(r) and max(p)<=40
    res['counter_size5']=bool(c) and c[0]['size']==5
    res['tail_unchanged']=bool(c) and math.dist(tip(c[0],dpr),[1096,495])<=3
    res['out_from_original']=rerender_matches(run)
    res['no_warnings']=rep['warnings']==[]
    return res
def s3(run):
    rep,A,dpr=load(run); res={}
    orange=(1375,479,1513,506); oblock=(1340,400,1600,598)
    sp=[a for a in A if a['type']=='spotlight']; ar=[a for a in A if a['type']=='arrow']
    res['spot_covers_text']=any(contains(s['rect'],orange) for s in sp)
    res['spot_tight']=any(s['rect'][2]<=420 and s['rect'][3]<=300 for s in sp)
    res['arrow_left_points_at']=any(a['from'][0]<a['to'][0] and dist_pt_box(a['to'],oblock)<60 for a in ar)
    res['no_warnings']=rep['warnings']==[]
    res['out_from_original']=rerender_matches(run)
    return res
def s4(run):
    rep,A,dpr=load(run); res={}
    c=[a for a in A if a['type']=='counter']; r=[a for a in A if a['type']=='rect']
    res['counter_size5']=bool(c) and c[0]['size']==5
    res['tail_unchanged']=bool(c) and math.dist(tip(c[0],dpr),[1358,95])<=3
    res['rect_unchanged']=bool(r) and r[0]['rect']==[1362.0,68.0,172.0,54.0] and r[0]['fill']=='outline'
    res['out_from_original']=rerender_matches(run)
    return res
def s5(run):
    rep,A,dpr=load(run); res={}
    h=[a for a in A if a['type']=='highlight']
    im=Image.open(f'{run}/out.png').convert('RGB')
    ys=[p[1] for a in h for p in a['points']]
    y=int(sum(ys)/len(ys)) if ys else 858
    res['covers_line']=bool(h) and 840<=y<=880
    res['color']=h[0]['color'] if h else None  # grey/white/black renders navy here: the reply must say so
    return res
run=sys.argv[2]
try: r={'s1':s1,'s2':s2,'s3':s3,'s4':s4,'s5':s5}[sys.argv[1]](run)
except Exception as e: r={'error':repr(e)}
print(json.dumps(r)); 
