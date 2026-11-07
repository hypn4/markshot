import json, sys, math, subprocess, os
from PIL import Image
D=[22,28,36,48,64,84,112,150,200,266]
EVAL_DIR=os.environ['EVAL_DIR']; BIN=f'{EVAL_DIR}/bin/markshot'; SHOT=f'{EVAL_DIR}/shot.png'; CONSOLE=f'{EVAL_DIR}/console.png'
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
def rerender_matches(run,shot=SHOT):
    subprocess.run([BIN,'render',shot,'--spec','-','-o',f'{run}/_re.png'],input=json.dumps(json.load(open(f'{run}/report.json'))['spec']).encode(),capture_output=True,check=True)
    return open(f'{run}/_re.png','rb').read()==open(f'{run}/out.png','rb').read()
def frames_only(run,A,dpr,shot):  # the rects and ovals alone with the dimming at 0: where their lines fall
    spec={'dpr':dpr,'spotlight_dim':0,'annotations':[a for a in A if a['type'] in('rect','oval')]}
    subprocess.run([BIN,'render',shot,'--spec','-','-o',f'{run}/_frames.png'],input=json.dumps(spec).encode(),capture_output=True,check=True)
    return Image.open(f'{run}/_frames.png').convert('RGB')
def out_full(run,rep):  # out.png where it sits in the input, magenta where a crop left it out
    out=Image.open(f'{run}/out.png').convert('RGB'); c=rep['spec'].get('crop')
    if not c: return out
    full=Image.new('RGB',(rep['width'],rep['height']),(255,0,255)); full.paste(out,(int(c[0]),int(c[1]))); return full
def same(x,y,b): return x.crop(b).tobytes()==y.crop(b).tobytes()  # b=(x0,y0,x1,y1)
def box(r): return (r[0],r[1],r[0]+r[2],r[1]+r[3])
def on_circle(c,b,dpr): return dist_pt_box(c['center'],b)<D[c['size']]/2*dpr  # the badge's circle reaches into b
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
    sp=[a for a in A if a['type']=='spotlight' or a.get('spotlight')]; ar=[a for a in A if a['type']=='arrow']
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
def s6(run):
    rep,A,dpr=load(run); res={}
    green=(1375,80,1519,105); shot=f'{EVAL_DIR}/shot-nodpi.png'
    ov=[a for a in A if a['type']=='oval' and a.get('spotlight') and contains(a['rect'],green)]
    sp=[a for a in A if a['type']=='spotlight']
    res['dpr_2']=dpr==2
    res['oval_dims_the_rest']=bool(ov)
    res['no_second_spotlight']=not sp
    a=Image.open(f'{run}/out.png').convert('RGB').crop(green); b=Image.open(shot).convert('RGB').crop(green)
    res['text_untouched']=a.tobytes()==b.tobytes()
    res['no_warnings']=rep['warnings']==[]
    res['out_from_original']=rerender_matches(run,shot)
    return res
def s7(run):
    rep,A,dpr=load(run); res={}
    seg=(1954,672,2397,736); email=(1954,1200,2199,1242); audit=(1953,1314,2256,1356)  # button group; switch + label
    seg_text=(1986,694,2367,719)  # the group's three labels
    near=[(1955,624,2566,649),(1955,785,2135,807),(1954,1086,2300,1128),(2047,1144,2593,1169),(2048,1258,2550,1283),(2047,1372,2569,1393),(1954,1428,2269,1470)]  # descriptions and rows next to them
    T=[seg,email,audit]; fr=[a for a in A if a['type'] in('rect','oval')]
    F=[[f for f in fr if contains(f['rect'],t) and not any(contains(f['rect'],u) for u in T if u!=t)] for t in T]
    res['dpr_2']=dpr==2
    res['each_target_framed']=all(F)
    res['frames_dim_the_rest']=all(any(f.get('spotlight') for f in fs) for fs in F)
    res['no_second_spotlight']=not [a for a in A if a['type']=='spotlight']
    cs=sorted([a for a in A if a['type']=='counter'],key=lambda a:a['number'])
    anchor=[box(fs[0]['rect']) if fs else t for fs,t in zip(F,T)]  # a badge marks its target's frame
    res['badges_1_to_3']=[c['number'] for c in cs]==[1,2,3] and all(min(range(3),key=lambda i:dist_pt_box(tip(c,dpr),anchor[i]))==k and dist_pt_box(tip(c,dpr),anchor[k])<100 for k,c in enumerate(cs))
    out=out_full(run,rep); orig=Image.open(CONSOLE).convert('RGB')
    res['targets_untouched']=all(same(out,orig,b) for b in (seg_text,email,audit))
    fo=frames_only(run,A,dpr,CONSOLE)
    res['frames_clear_neighbours']=all(same(fo,orig,b) for b in near)
    res['toggle_frames']=[fs[0]['type'] if fs else None for fs in F[1:]]  # the user asked for hand-drawn ovals: a rect needs a reason in the reply
    res['no_warnings']=rep['warnings']==[]
    res['out_from_original']=rerender_matches(run,CONSOLE)
    return res
def s8(run):
    rep,A,dpr=load(run); res={}
    pills=[(1178,584,1329,624),(1178,656,1348,696),(1178,728,1332,768),(1178,801,1350,841)]  # Status, rows 1-4
    near=[(547,526,614,545),(971,525,1050,549),(1179,526,1254,545),(1419,526,1532,545),(1591,525,1726,549),  # header
          (547,593,699,618),(971,592,1142,612),(1419,593,1472,618),(1591,593,1708,618),
          (547,665,717,690),(972,664,1099,684),(1419,665,1471,690),(1591,665,1716,690),
          (547,737,733,758),(971,736,1113,760),(1419,737,1471,762),(1590,737,1709,762),
          (547,810,718,831),(971,809,1099,829),(1419,810,1468,835),(1590,811,1698,831)]  # Name, Region, Instances, Last deploy
    cs=sorted([a for a in A if a['type']=='counter'],key=lambda a:a['number'])
    res['dpr_2']=dpr==2
    res['badges_1_to_4']=[c['number'] for c in cs]==[1,2,3,4]
    res['badges_mark_rows']=len(cs)==4 and all(min(range(4),key=lambda i:dist_pt_box(tip(c,dpr),pills[i]))==k and min(dist_pt_box(tip(c,dpr),pills[k]),dist_pt_box(c['center'],pills[k])-D[c['size']]/2*dpr)<40 for k,c in enumerate(cs))
    res['badges_off_status']=not any(on_circle(c,b,dpr) for c in cs for b in pills)
    res['badges_off_neighbours']=not any(on_circle(c,b,dpr) for c in cs for b in near)
    res['no_warnings']=rep['warnings']==[]
    res['out_from_original']=rerender_matches(run,CONSOLE)
    return res
run=sys.argv[2]
try: r={'s1':s1,'s2':s2,'s3':s3,'s4':s4,'s5':s5,'s6':s6,'s7':s7,'s8':s8}[sys.argv[1]](run)
except Exception as e: r={'error':repr(e)}
print(json.dumps(r)); 
