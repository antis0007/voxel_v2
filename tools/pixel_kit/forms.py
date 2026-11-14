"""Workshop wand, closed grip and loose cuff. Silhouette coordinates are metres."""
from mesh import shape,box

def hand():
    shape('Palm and thenar mass',[(.013,-.027),(.029,-.043),(.069,-.045),
        (.085,-.021),(.074,.045),(.054,.069),(.030,.064),(.016,.035)],
        .015,-.027,'skin','hand',bevel=.003,bulge=.012)
    # Each finger curls in depth around the handle; its inner contour stays open.
    # Tightly spaced metacarpals read as one fist, with a smaller little finger.
    profile=[(.038,-.020),(.012,-.031),(-.015,-.026),(-.032,-.009),
        (-.034,.012),(-.022,.030),(.004,.035),(.025,.027),(.029,.015),
        (.018,.009),(-.001,.016),(-.014,.013),(-.015,.002),
        (-.003,-.011),(.020,-.014)]
    for name,y,scale,bone in [('Index',.039,1.,1),('Middle',.016,1.02,2),
                            ('Ring',-.006,.94,3),('Little',-.025,.80,4)]:
        outline=[(x*scale+.004*(1-scale),z*scale) for x,z in profile]
        shape(name+' closed curl',outline,y+.0105,y-.0105,'skin','hand',
              bone,bevel=.0015,plane='xz')
    # Opposing thumb crosses the index, with a visible web and bent distal pad.
    shape('Opposing thumb',[(.057,.057),(.044,.069),(.028,.067),(-.009,.046),
        (-.015,.034),(-.007,.024),(.005,.025),(.028,.045),(.049,.041)],
        .043,.022,'skin','hand',5,bevel=.003,bulge=.003)
    shape('Wrist',[(.057,-.045),(.086,-.055),(.100,-.034),(.078,-.014)],
        .014,-.024,'skin','hand',bevel=.003)

def sleeve():
    # Local +Y ends at the wrist. A single side fold breaks the short taper.
    shape('Folded cuff silhouette',[(-.046,-.125),(.055,-.125),(.062,-.098),
        (.045,-.065),(.040,-.015),(-.035,-.012),(-.044,-.047),(-.055,-.063),
        (-.046,-.084)],.034,-.036,'cloth','sleeve',bevel=.003,bulge=.009)
    box('Leather wrist cuff',0,-.012,.078,.025,.033,-.033,'leather','sleeve',.003)

def shaft(i):
    g=f'shaft_{i}'
    bend=[-.007,.006,-.011,.012,-.003,.0][i]
    shape('Timber handle '+str(i),[(-.013,-.085),(.011,-.089),(.017,-.071),
        (.014,.067),(.012+bend,.142),(.020+bend,.235),(-.015+bend,.235),
        (-.019+bend,.146),(-.016,.068),(-.020,-.065)],.014,-.013,
        ['wood','ceramic','mineral','dark','copper','cloth'][i],g,bevel=.002)
    shape('Leather grip '+str(i),[(-.021,-.045),(.021,-.046),(.023,.066),
        (.016,.078),(-.019,.074)],.019,-.018,'leather',g,bevel=.002)

def head(i):
    g=f'head_{i}'
    # Broad ceramic socket, split timber jaws, recessed mineral. No crown.
    shape('Ceramic socket '+str(i),[(-.016,-.026),(.020,-.026),(.035,.018),
        (.032,.039),(.014,.028),(-.018,.032),(-.032,.043),(-.034,.010)],
        .022,-.019,'ceramic',g,bevel=.002)
    left=[(-.029,.018),(-.014,.023),(-.012,.067),(-.020,.098),(-.033,.113),
          (-.040,.096),(-.036,.051)]
    right=[(.009,.023),(.028,.019),(.039,.052),(.034,.089),(.019,.101),
           (.021,.070),(.015,.050)]
    if i==0:
        shape('Left split timber',left,.015,-.012,'wood',g,bevel=.002)
        shape('Right split timber',right,.015,-.012,'wood',g,bevel=.002)
        shape('Recessed mineral',[(-.008,.045),(.003,.042),(.011,.058),
            (.006,.076),(-.004,.081),(-.012,.064)],.010,-.006,'mineral',g,bevel=.001)
    elif i==1:
        shape('Single resonator',[(-.022,.027),(.014,.026),(.027,.066),
            (.005,.125),(-.021,.088),(-.030,.058)],.015,-.015,
            'mineral',g,bevel=.001,bulge=.012)
    elif i==2:
        for x,y,s in [(-.020,.025,1.),(.018,.020,.76)]:
            shape('Paired prism '+str(x),[(x-.012,y),(x+.012,y),(x+.016,y+.047*s),
                (x,y+.097*s),(x-.016,y+.050*s)],.014,-.012,
                'mineral',g,bevel=.001,bulge=.006)
    elif i==3:
        from math import sin,cos,pi
        for j in range(8):
            a=j*pi/4; b=(j+1)*pi/4
            outline=[(r*cos(t),.067+r*sin(t)) for r,t in
                     [(.044,a),(.044,b),(.032,b),(.032,a)]]
            shape('Copper resonant hoop '+str(j),outline,.009,-.009,'copper',g,bevel=0)
        box('Suspended seed',0,.067,.014,.025,.007,-.007,'mineral',g)
    elif i==4:
        shape('Flame chamber',[(-.025,.025),(.025,.025),(.031,.059),
            (.017,.100),(.010,.075),(-.004,.129),(-.014,.092),(-.029,.067)],
            .008,-.010,'mineral',g,bevel=.001,bulge=.006)
    else:
        shape('Watching housing',[(-.043,.064),(-.022,.087),(.012,.091),
            (.043,.066),(.020,.045),(-.018,.043)],.018,-.016,'dark',g,bevel=.002)
        shape('Iris',[(-.012,.049),(.011,.049),(.018,.066),(.010,.084),
            (-.010,.084),(-.018,.065)],.024,.019,'mineral',g,bevel=.001)
        box('Vertical aperture',0,.067,.004,.025,.027,.024,'dark',g,.0005)
    box('Copper repair strap '+str(i),0,.009,.073,.014,.025,-.022,'copper',g,.001)
    box('Single strap rivet '+str(i),.020,.009,.007,.007,.030,.025,'dark',g,.001)
