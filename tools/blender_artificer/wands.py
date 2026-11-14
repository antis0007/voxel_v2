"""Workshop construction: fitted timber, wrapped grips and open metal settings."""
from math import sin, cos, pi
from geometry import loft, band, tube, ellipsoid, gem

def shaft(collection, style):
    pigments=['wood','bone','crystal','obsidian','patina','starwood']
    profiles=[
        [(0,-.043,.016), (0,.090,.015),(.006,.135,.025),(.008,.18,.023),
         (-.003,.235,.019),(-.006,.278,.015),(0,.34,.013)],
        [(0,-.043,.016),(0,.09,.014),(.004,.145,.017),(.006,.17,.020),
         (.005,.195,.012),(.003,.285,.010),(0,.32,.016),(0,.34,.010)],
        [(0,-.043,.016),(0,.09,.015),(.001,.14,.019),(.003,.22,.017),
         (.004,.27,.011),(0,.34,.007)],
        [(0,-.043,.016),(0,.09,.015),(.004,.16,.020),(-.006,.21,.017),
         (.005,.27,.011),(0,.34,.008)],
        [(0,-.043,.015),(0,.10,.015),(0,.13,.019),(0,.16,.019),
         (0,.165,.011),(0,.31,.011),(0,.34,.010)],
        [(0,-.043,.016),(0,.09,.015),(-.009,.17,.018),(-.014,.24,.013),
         (-.009,.28,.011),(0,.34,.009)],
    ][style]
    loft('Shaft - '+pigments[style], [((x,y,0),r,r*.82) for x,y,r in profiles],
         pigments[style],collection,8 if style in (2,3) else 20)
    # Tangential leather edges actually spiral; the grip remains narrow enough
    # for the hand pads to contact it, rather than floating above a block collar.
    loft('Grip leather base',[((0,-.030,0),.0155,.0135),((0,.092,0),.0155,.0135)],
         'wrap',collection)
    points=[]
    for i in range(80):
        t=i/79
        a=t*2*pi*3
        points.append((.0162*cos(a),-.027+t*.116,.0142*sin(a)))
    tube('Three leather turns',points,.0025,'leather',collection,6)
    band('Pommel seat',-.037,.018,.014,'iron',collection,.84)
    band('Pommel brass lip',-.044,.0185,.004,'brass_edge',collection,.84)
    band('Grip upper ferrule',.100,.018,.014,'brass',collection,.84)
    band('Ferrule lip',.108,.0187,.0035,'brass_edge',collection,.84)
    # Maker's plate and pins give a purpose to the broad section above the grip.
    loft('Gauge backing',[((.004,.119,.023),.013,.003),((.005,.177,.023),.012,.003)],
         'patina',collection,8)
    if style==4:
        for side in (-1,1):
            tube('Copper conductor',[(side*.013,.16,0),(side*.013,.31,0)],.0026,'brass',collection)
        for y in (.174,.292):
            band('Copper joint',y,.013,.009,'iron',collection)
        for i in range(3):
            band('Wound copper coil',.210+i*.014,.014,.006,'brass',collection)
    band('Head socket lower collar',.322,.014,.012,'iron',collection)
    band('Head socket shoulder',.333,.016,.008,'brass',collection)
    band('Socket rim',.339,.0165,.0035,'brass_edge',collection)

def head(collection, style):
    band('Setting foot',-.006,.014,.011,'brass',collection)
    if style==0:
        gem('Cut stone', (0,.040,0),(.030,.046,.021),'jade',collection,6)
        gem('Spark channel',(0,.034,.021),(.004,.016,.003),'jewel',collection,4)
        # Repeated arches borrow the reference's framed openings. The gem stays
        # visible through the setting and the claws actually meet its shoulders.
        for side in (-1,1):
            points=[(side*r,y,0) for r,y in
                    [(.013,-.012),(.034,.005),(.043,.031),(.039,.061),(.027,.078)]]
            tube('Forged focusing jaw',points,[.007,.008,.007,.006,.0045],
                 'brass',collection,8)
        band('Ceramic isolator',-.003,.023,.015,'bone',collection,.82)
        band('Setting rim',.010,.026,.007,'brass_edge',collection,.82)
        ellipsoid('Replaceable jaw pin',(-.034,.009,.005),(.006,.006,.004),
                  'iron',collection)
    elif style==1:
        gem('Long cut crystal',(0,.042,0),(.024,.059,.019),'jewel',collection)
        for side in (-1,1):
            tube('Crystal shoe',[(side*.008,-.008,0),(side*.022,.012,0),(side*.020,.030,0)],
                 [.004,.003,.002],'brass',collection)
    elif style==2:
        gem('Left prism',(-.022,.028,0),(.014,.038,.012),'jewel',collection)
        gem('Right prism',(.023,.045,0),(.015,.049,.012),'jewel',collection)
        tube('Forked cradle',[(-.022,.005,0),(-.012,-.005,0),(0,-.010,0),(.014,0,0),(.023,.009,0)],
             .004,'brass',collection)
    elif style==3:
        points=[(.044*cos(i*pi/24),.039+.044*sin(i*pi/24),0) for i in range(49)]
        tube('Open instrument hoop',points,.004,'brass',collection,8)
        gem('Suspended stone',(0,.039,0),(.010,.016,.008),'jewel',collection)
        tube('Suspension',[(0,.055,0),(0,.080,0)],.003,'iron',collection,6)
    elif style==4:
        loft('Flame stone',[((0,-.002,0),.012,.009),((-.008,.020,0),.024,.017),
                            ((-.004,.042,0),.019,.013),((.012,.061,0),.010,.007),
                            ((.008,.081,0),.0007,.0007)],'jewel',collection,7)
        for side in (-1,1):
            tube('Flame claw',[(0,-.004,0),(side*.022,.015,0),(side*.016,.028,0)],
                 [.005,.003,.0017],'brass',collection)
    else:
        gem('Eye stone',(0,.026,0),(.014,.023,.009),'jewel',collection,8)
        for side in (-1,1):
            tube('Eye frame',[(x,.026+side*y,0) for x,y in
                             [(-.039,0),(-.022,.016),(0,.025),(.022,.016),(.039,0)]],
                 [.002,.0035,.004,.0035,.002],'brass',collection)
        ellipsoid('Pupil',(0,.026,.009),(.0035,.014,.0025),'obsidian',collection)
