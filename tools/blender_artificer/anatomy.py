"""Compact power grip and a bent, gathered work sleeve."""
from math import pi
from geometry import loft, tube

# Shared by the Blender armature and offline skin-weight export.
PIVOTS = [(.030,.061,-.017),(.033,.042,-.019),(.034,.023,-.018),
          (.035,.005,-.014),(.048,.006,.011)]
WRIST = (.050,-.026,.004)


def skin_weight(bone,p):
    if 1 <= bone <= 4:
        return max(0.,min(1.,(PIVOTS[bone-1][0]-p.x)/.028))
    if bone == 5:
        return max(0.,min(1.,(p.y-.012)/.033))
    return 0.


def hand(collection):
    loft('Palm - sloping metacarpal wedge', [
        ((.053,-.041,.004),.016,.014),
        ((.047,-.020,.002),.019,.017),
        ((.037,.004,-.004),.022,.021),
        ((.030,.034,-.010),.022,.019),
        ((.023,.057,-.014),.017,.015),
        ((.018,.066,-.015),.010,.010)],
        'skin',collection,8,pi/8)
    # Each finger closes farther down the handle. The middle knuckle leads;
    # the little finger is tucked, so the pads do not form four parallel bars.
    fingers = [
        ('Index',[(.030,.061,-.017),(.006,.065,-.027),(-.017,.060,-.022),
                  (-.026,.053,-.006),(-.019,.046,.016),(.001,.045,.020)],
                 [.009,.010,.010,.009,.008,.005]),
        ('Middle',[(.033,.042,-.019),(.006,.045,-.028),(-.020,.038,-.022),
                   (-.028,.031,-.004),(-.017,.024,.018),(.004,.024,.019)],
                  [.010,.011,.011,.010,.009,.0055]),
        ('Ring',[(.034,.023,-.018),(.009,.025,-.027),(-.018,.018,-.022),
                 (-.025,.011,-.004),(-.014,.004,.016),(.006,.004,.017)],
                [.0095,.010,.010,.009,.008,.005]),
        ('Little',[(.035,.005,-.014),(.014,.007,-.023),(-.009,.000,-.021),
                   (-.018,-.008,-.006),(-.009,-.014,.012),(.009,-.012,.015)],
                  [.008,.0085,.0085,.008,.007,.0045]),
    ]
    for name,points,radii in fingers:
        tube(name+' - closed phalange chain',points,radii,'skin',collection,8)
    # The saddle enters from the wrist side. A distinct thumb crosses the index
    # pad; its end lies against the grip instead of pointing up beside it.
    loft('Thenar - thumb saddle',[
        ((.048,-.014,.009),.012,.014),
        ((.048,.008,.015),.016,.017),
        ((.040,.027,.021),.014,.014),
        ((.031,.039,.022),.008,.008)],'skin',collection,8,pi/8)
    tube('Thumb - opposed across pads',[
        (.048,.006,.011),(.047,.025,.024),(.035,.041,.031),
        (.021,.046,.035),(.004,.035,.029),(-.005,.028,.020)],
        [.012,.013,.012,.011,.010,.0065],'skin',collection,8)


def sleeve(collection):
    # Broad polygonal folds change the centreline as well as its width.
    # The wrist opening is local zero; the engine attaches it to WRIST.
    loft('Wool - bent forearm and gathered cuff',[
        ((-.014,-.280,-.021),.043,.034),
        ((-.017,-.145,-.025),.046,.037),
        ((-.007,-.110,-.018),.041,.033),
        ((.003,-.084,-.010),.046,.037),
        ((.006,-.065,-.004),.034,.029),
        ((.007,-.050,.001),.040,.032),
        ((.003,-.031,.002),.030,.026),
        ((0,-.012,0),.026,.023)],'cloth',collection,8,pi/8)
    loft('Cuff - broad turned linen',[
        ((0,-.025,0),.032,.027),
        ((0,-.018,0),.034,.029),
        ((0,.003,0),.030,.026),
        ((0,.009,0),.026,.022)],'cuff',collection,8,pi/8)
    loft('Cuff - stitched lower piping',[
        ((0,-.026,0),.0325,.0275),
        ((0,-.020,0),.0345,.0295)],'seam',collection,8,pi/8)
