"""Author the held kit as lofted low-poly surfaces with local paint coordinates."""
from pathlib import Path
from math import sin, cos, pi, sqrt

ROOT = Path(__file__).resolve().parents[1] / 'assets' / 'viewmodel'
SKIN = ((.60,.43,.32),54,0.,0)
CLOTH = ((.18,.25,.26),54,.15,0)
LINING = ((.27,.31,.29),54,.15,0)
LEATHER = ((.20,.15,.12),54,.25,0)
BRASS = ((.48,.37,.23),55,.4,0)
EDGE = ((.65,.53,.33),55,.4,0)
WRAP = ((.80,.80,.80),55,.1,1)
JEWEL = ((.52,.61,.64),55,.8,2)
WOOD = ((.31,.22,.15),55,0.,0)
BONE = ((.65,.61,.50),55,.2,0)
CRYSTAL = ((.31,.52,.56),55,.8,0)
OBSIDIAN = ((.085,.095,.13),55,.6,0)
COPPER = ((.25,.40,.33),55,.4,0)
STARWOOD = ((.18,.19,.27),55,0.,0)

def add(a,b): return tuple(x+y for x,y in zip(a,b))
def sub(a,b): return tuple(x-y for x,y in zip(a,b))
def mul(a,k): return tuple(x*k for x in a)
def dot(a,b): return sum(x*y for x,y in zip(a,b))
def cross(a,b): return (a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])
def unit(a): return mul(a,1/max(sqrt(dot(a,a)),1e-8))

class Sculpt:
    def __init__(self): self.vertices=[]

    def triangle(self, points, uvs, material, tones=(1,1,1), outward=None):
        normal=unit(cross(sub(points[1],points[0]),sub(points[2],points[0])))
        if outward is not None and dot(normal,outward)<0:
            points=[points[0],points[2],points[1]]
            uvs=[uvs[0],uvs[2],uvs[1]]
            tones=(tones[0],tones[2],tones[1])
            normal=mul(normal,-1)
        color,kind,finish,tint=material
        for point,uv,tone in zip(points,uvs,tones):
            self.vertices.append((point,normal,tuple(min(.99,c*tone) for c in color),uv,kind,finish,tint))

    def loft(self, profiles, material, sides=8, folds=0., phase=pi/8):
        # (centre, across radius, depth radius); cross-sections follow the spine.
        rings=[]; axes=[]; distances=[0.]
        for i,(p,rx,rz) in enumerate(profiles):
            tangent=unit(sub(profiles[min(i+1,len(profiles)-1)][0],profiles[max(i-1,0)][0]))
            reference=(1,0,0) if abs(tangent[0])<.85 else (0,1,0)
            across=unit(sub(reference,mul(tangent,dot(reference,tangent))))
            depth=unit(cross(across,tangent))
            ring=[]
            for j in range(sides):
                a=j*2*pi/sides+phase
                fold=1+folds*sin(3*a+i*.8)
                ring.append(add(p,add(mul(across,cos(a)*rx*fold),mul(depth,sin(a)*rz*fold))))
            rings.append(ring); axes.append(tangent)
            if i: distances.append(distances[-1]+sqrt(dot(sub(p,profiles[i-1][0]),sub(p,profiles[i-1][0]))))
        for i in range(len(rings)-1):
            for j in range(sides):
                k=(j+1)%sides
                points=[rings[i][j],rings[i+1][j],rings[i+1][k],rings[i][k]]
                circumference=2*pi*(profiles[i][1]+profiles[i][2])*.5
                uvs=[(j/sides*circumference,distances[i]),(j/sides*circumference,distances[i+1]),
                     ((j+1)/sides*circumference,distances[i+1]),((j+1)/sides*circumference,distances[i])]
                outward=sub(mul(add(points[0],points[1]),.5),mul(add(profiles[i][0],profiles[i+1][0]),.5))
                # Broad pigment planes, not a highlight on every geometric edge.
                tone=1+.035*cos(j*2*pi/sides)-.025*sin(i*1.7+j)
                for indices in [(0,1,2),(0,2,3)]:
                    self.triangle([points[q] for q in indices],[uvs[q] for q in indices],material,
                                  (tone,tone,tone),outward)
        for i in [0,len(rings)-1]:
            for j in range(sides):
                self.triangle([profiles[i][0],rings[i][j],rings[i][(j+1)%sides]],
                    [(0,0),(0,.02),(.02,.02)],material,outward=mul(axes[i],-1 if i==0 else 1))

    def ring(self, y, radius, width, material=BRASS, cx=0, cz=0):
        self.loft([((cx,y-width/2,cz),radius*.92,radius*.92),
                   ((cx,y-width/3,cz),radius,radius),
                   ((cx,y+width/3,cz),radius,radius),
                   ((cx,y+width/2,cz),radius*.92,radius*.92)],material,10)

    def gem(self, center, rx, ry, rz, material=JEWEL):
        x,y,z=center
        self.loft([((x,y-ry,z),rx*.18,rz*.18),((x,y-ry*.38,z),rx*.8,rz*.8),
                   ((x,y+ry*.1,z),rx,rz),((x,y+ry*.65,z),rx*.58,rz*.58),
                   ((x,y+ry,z),rx*.04,rz*.04)],material,6,phase=0.)

    def save(self,name):
        lines=['// Sculpted surface; metres, local UVs, sRGB pigment and material finish.','(vertices: [']
        def fmt(values): return '('+', '.join(f'{v:.6f}' for v in values)+')'
        for p,n,c,uv,kind,finish,tint in self.vertices:
            lines.append(f'    (p: {fmt(p)}, n: {fmt(n)}, c: {fmt(c)}, uv: {fmt(uv)}, kind: {kind}.0, finish: {finish:.2f}, tint: {tint}),')
        lines+= ['])','']
        (ROOT/f'{name}.ron').write_text('\n'.join(lines),encoding='utf-8')
        print(name,len(self.vertices)//3,'triangles')

hand=Sculpt()
# The back slopes into the knuckles; the heel narrows into the wrist.
hand.loft([((.030,-.045,-.002),.019,.018),((.034,-.018,-.003),.025,.021),
           ((.034,.014,-.005),.030,.023),((.032,.049,-.006),.029,.023),
           ((.027,.074,-.004),.024,.019),((.023,.084,-.001),.016,.012)],SKIN,8)
# Each finger arcs around the handle with three knuckles and a tapered fingertip.
for i,(y,r) in enumerate([(.012,.009),(.032,.010),(.052,.010),(.071,.009)]):
    hand.loft([((.036,y,-.023),r*.95,r),((.008,y+.002,-.027),r,r),
               ((-.020,y+.003,-.019),r*1.02,r*.94),((-.025,y+.001,.004),r*.85,r*.85),
               ((-.010,y-.002,.020),r*.70,r*.74),((.001,y-.004,.019),r*.40,r*.45)],SKIN,7)
# The thumb crosses diagonally over the index finger, rather than adding another bar.
hand.loft([((.048,.034,.012),.014,.012),((.039,.060,.025),.013,.011),
           ((.019,.080,.027),.011,.010),((-.002,.080,.022),.009,.008),
           ((-.008,.074,.016),.004,.005)],SKIN,7)
hand.save('hand')

sleeve=Sculpt()
sleeve.loft([((.010,-.245,-.012),.047,.038),((.006,-.19,-.008),.045,.037),
             ((.008,-.13,.002),.040,.032),((.002,-.084,.001),.036,.030),
             ((0,-.055,0),.034,.027),((0,-.031,0),.037,.029)],CLOTH,10,.10)
# Turned cuff lining and compressed folds nearest the wrist.
sleeve.loft([((0,-.047,0),.038,.030),((0,-.034,0),.039,.031),
             ((0,-.025,0),.031,.025),((0,-.015,0),.024,.020),
             ((0,0,0),.023,.020)],LINING,10,.035)
sleeve.loft([((0,-.046,0),.039,.031),((0,-.038,0),.039,.031)],LEATHER,10)
sleeve.save('sleeve')

for style,material in enumerate([WOOD,BONE,CRYSTAL,OBSIDIAN,COPPER,STARWOOD]):
    shaft=Sculpt()
    if style==0:
        profiles=[((0,-.040,0),.016,.014),((0,.09,0),.015,.013),((.006,.15,0),.017,.012),
                  ((-.004,.21,.003),.013,.011),((-.009,.265,.002),.011,.010),((0,.34,0),.009,.009)]
    elif style==1:
        profiles=[((0,-.04,0),.017,.014),((0,.1,0),.014,.012),((.006,.16,0),.017,.014),
                  ((.008,.18,0),.012,.010),((.004,.26,0),.010,.010),((0,.31,0),.015,.013),((0,.34,0),.009,.009)]
    elif style==2:
        profiles=[((0,-.04,0),.015,.012),((0,.11,0),.016,.013),((.003,.18,0),.018,.013),
                  ((.004,.25,0),.013,.009),((0,.34,0),.006,.005)]
    elif style==3:
        profiles=[((0,-.04,0),.017,.012),((0,.13,0),.017,.012),((.008,.18,0),.020,.011),
                  ((-.007,.24,0),.016,.010),((.005,.28,0),.012,.008),((0,.34,0),.007,.006)]
    elif style==4:
        profiles=[((0,-.04,0),.014,.014),((0,.105,0),.014,.014),((0,.11,0),.018,.018),
                  ((0,.15,0),.018,.018),((0,.155,0),.011,.011),((0,.32,0),.011,.011),((0,.34,0),.009,.009)]
    else:
        profiles=[((0,-.04,0),.015,.012),((0,.11,0),.015,.012),((-.008,.18,0),.016,.012),
                  ((-.012,.26,0),.011,.009),((0,.34,0),.008,.007)]
    shaft.loft(profiles,material,6 if style==2 else 5 if style==3 else 9)
    # Leather is one continuous wrap; shallow seams are painted in local UVs.
    shaft.loft([((0,-.025,0),.0165,.0145),((0,.09,0),.0165,.0145),
                ((0,.105,0),.014,.013)],WRAP,9)
    shaft.ring(-.034,.017,.008)
    shaft.ring(.11,.018,.009)
    shaft.ring(.327,.012,.011)
    shaft.ring(.337,.014,.007,EDGE)
    if style==4:
        for x in [-.012,.012]:
            shaft.loft([((x,.16,0),.0025,.0025),((x,.31,0),.0025,.0025)],BRASS,5)
    shaft.save(f'shaft_{style}')

for style in range(6):
    head=Sculpt()
    if style==0:
        head.gem((0,.024,0),.027,.037,.022)
        for x in [-.024,.024]:
            head.loft([((x*.3,-.006,0),.004,.004),((x,.012,0),.003,.004),
                       ((x,.030,0),.002,.003)],BRASS,6)
    elif style==1:
        head.gem((0,.035,0),.023,.052,.018)
    elif style==2:
        head.gem((-.023,.024,0),.014,.036,.012)
        head.gem((.022,.040,0),.015,.045,.012)
        head.loft([((-.023,-.004,0),.004,.004),((0,-.008,0),.005,.005),
                   ((.022,0,0),.004,.004)],BRASS,6)
    elif style==3:
        points=[((.044*cos(i*pi/10),.036+.044*sin(i*pi/10),0),.004,.004) for i in range(21)]
        head.loft(points,BRASS,5)
        head.gem((0,.036,0),.010,.015,.009)
        head.loft([((0,-.009,0),.006,.006),((0,-.004,0),.005,.005)],BRASS,6)
    elif style==4:
        head.gem((-.007,.022,0),.022,.032,.018)
        head.gem((.010,.052,0),.010,.022,.010)
    else:
        head.loft([((-.04,.024,0),.002,.002),((-.02,.024,0),.018,.008),
                   ((.01,.024,0),.021,.010),((.04,.024,0),.002,.002)],BONE,8)
        head.gem((0,.024,.012),.009,.015,.005,OBSIDIAN)
        head.gem((0,.024,.017),.003,.010,.002)
    if style!=3: head.ring(-.005,.013,.006)
    head.save(f'head_{style}')
