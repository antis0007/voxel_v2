export const roles = ["Apartments", "Clinic", "Offices", "Workshop", "Relay"];
export const roomTypes = {
    Hall: "#c2905a", Passage: "#72989f", Living: "#927ca1", Workshop: "#a36b51",
    Store: "#667f98", Library: "#8d8261", Shrine: "#547f72", Crypt: "#777789",
};
export const colors = {
    Air: "#202a35", Concrete: "#a1a6a2", DressedStone: "#c9bfaa", WeatheredBricks: "#8e695b",
    StoneBricks: "#9f9f97", Limestone: "#d2c4a9", LimePlaster: "#ddcba8", CarvedStone: "#898f8f",
    Basalt: "#505d68", RustedSteel: "#a46247", SteelPlate: "#63798a", WindowGlass: "#70a8bc",
    GlazedTiles: "#56a5a0", Planks: "#9a7753", Log: "#765a42", RoofTiles: "#a85b46",
    CopperBlock: "#c27758", BronzeBlock: "#ac8347", Crystal: "#67cbbd", Bed: "#9c696d",
    Ice: "#a5c6d5", Cobblestone: "#898982",
};
export const materials = Object.keys(colors).filter(n => !["Air", "Bed"].includes(n));
const styles = {
    Apartments: ["Hip", "Panes", 3, ["WeatheredBricks", "DressedStone", "WeatheredBricks", "Planks", "RoofTiles", "WindowGlass"]],
    Clinic: ["Flat", "Panes", 2, ["LimePlaster", "Concrete", "WeatheredBricks", "GlazedTiles", "SteelPlate", "WindowGlass"]],
    Offices: ["Flat", "Glazed", 6, ["Concrete", "SteelPlate", "WeatheredBricks", "DressedStone", "SteelPlate", "WindowGlass"]],
    Workshop: ["Gable", "Panes", 1, ["WeatheredBricks", "RustedSteel", "WeatheredBricks", "Concrete", "SteelPlate", "WindowGlass"]],
    Relay: ["Lean", "Slits", 2, ["Basalt", "SteelPlate", "CarvedStone", "DressedStone", "BronzeBlock", "WindowGlass"]],
};

export function roomAt(d, x, z) {
    return d.rooms.findIndex(({box: [a,b,c,e]}) => x >= a && z >= b && x < c && z < e);
}
const room = (role, x, z, right, back) => ({ role, box: [x,z,right,back] });

export function template(role, nx = 6, nz = 4) {
    const [roof, windows, floors, palette] = styles[role];
    const d = { role, nx, nz, bay: 4, height: 4, floors, roof, windows, palette: [...palette],
        stairs: null, age: 0.08, seed: 424242, rooms: [], doors: [], entrances: [] };
    if (nx < 4 || nz < 3) {
        d.rooms.push(room(role === "Workshop" ? "Workshop" : "Hall", 0, 0, nx, nz));
    } else if (role === "Workshop") {
        const mid = Math.floor(nx / 2);
        d.rooms.push(room("Workshop", 0, 0, nx, nz-1), room("Store", 0, nz-1, mid, nz), room("Store", mid, nz-1, nx, nz));
    } else {
        const left = Math.max(1, Math.floor(nx/2)-1);
        const front = ["Clinic", "Offices"].includes(role) ? 1 : 0;
        if (front) d.rooms.push(room("Hall", 0, 0, nx, front));
        d.rooms.push(room("Passage", left, front, left+2, nz));
        const use = role === "Offices" ? "Library" : role === "Relay" ? "Shrine" : "Living";
        for (let z=front; z<nz; z++) {
            d.rooms.push(room(use, 0, z, left, z+1));
            if (left+2 < nx) d.rooms.push(room(use, left+2, z, nx, z+1));
        }
    }
    d.entrances = [["Front", Math.floor(nx/2)], ["Back", Math.floor(nx/2)]];
    autoDoors(d);
    autoStairs(d);
    if (!d.stairs) d.floors = 1;
    return d;
}

function cutRoom(r, [a,b,c,e]) {
    const [x,z,w,h] = r.box;
    const left = Math.max(x,a), front = Math.max(z,b), right = Math.min(w,c), back = Math.min(h,e);
    if (left >= right || front >= back) return [r];
    return [
        [x,z,left,h], [right,z,w,h], [left,z,right,front], [left,back,right,h],
    ].filter(([x0,z0,x1,z1]) => x0 < x1 && z0 < z1).map(box => ({ role: r.role, box }));
}

export function putRoom(d, box, role) {
    d.rooms = d.rooms.flatMap(r => cutRoom(r, box));
    d.rooms.push({ role, box });
    trimOpenings(d);
    return d.rooms.length-1;
}

export function fillGaps(d, role) {
    for (let z=0; z<d.nz; z++) for (let x=0; x<d.nx; x++) {
        if (roomAt(d,x,z) !== -1) continue;
        let right=x+1, back=z+1;
        while (right < d.nx && roomAt(d,right,z) === -1) right++;
        while (back < d.nz && Array.from({length:right-x}, (_,i) => roomAt(d,x+i,back)).every(id => id === -1)) back++;
        d.rooms.push(room(role,x,z,right,back));
    }
}

function neighbours(d, [axis,x,z]) {
    return axis === "X" ? [roomAt(d,x-1,z), roomAt(d,x,z)] : [roomAt(d,x,z-1), roomAt(d,x,z)];
}
export function validDoor(d, door) {
    const [a,b] = neighbours(d, door);
    return a !== -1 && b !== -1 && a !== b;
}

export function trimOpenings(d) {
    d.doors = d.doors.filter(door => validDoor(d, door));
    d.entrances = d.entrances.filter(([side,n]) => n >= 0 && n < (["Left","Right"].includes(side) ? d.nz : d.nx));
    if (d.stairs && !validStair(d, ...d.stairs)) d.stairs = null;
}

export function resize(d, nx, nz) {
    d.nx = nx; d.nz = nz;
    d.rooms = d.rooms.map(({role,box:[x,z,w,h]}) => room(role,x,z,Math.min(nx,w),Math.min(nz,h)))
        .filter(({box:[a,b,c,e]}) => a<c && b<e);
    trimOpenings(d);
}

export function autoDoors(d) {
    const pairs = new Map();
    for (let z=0; z<d.nz; z++) for (let x=0; x<d.nx; x++) {
        for (const axis of ["X","Z"]) {
            const edge = [axis,x,z];
            if (!validDoor(d,edge)) continue;
            const [a,b] = neighbours(d,edge).sort((a,b) => a-b);
            const key = a + "/" + b;
            if (!pairs.has(key)) pairs.set(key, { a, b, edges: [] });
            pairs.get(key).edges.push(edge);
        }
    }
    const sets = d.rooms.map((_,i) => i);
    const root = i => { while (sets[i] !== i) i = sets[i]; return i; };
    d.doors = [];
    for (const {a,b,edges} of pairs.values()) {
        if (root(a) === root(b)) continue;
        sets[root(a)] = root(b);
        d.doors.push(edges[Math.floor(edges.length/2)]);
    }
}

export function validStair(d,x,z) {
    const n = roomAt(d,x,z);
    return n >= 0 && [[x+1,z],[x,z+1],[x+1,z+1]].every(([a,b]) => roomAt(d,a,b) === n);
}
export function autoStairs(d) {
    const fits = [];
    for (let x=0; x<d.nx-1; x++) for (let z=0; z<d.nz-1; z++) {
        if (validStair(d,x,z)) fits.push([x,z]);
    }
    const distance = ([x,z]) => Math.abs(x+1-d.nx/2) + Math.abs(z+1-d.nz/2);
    d.stairs = fits.sort((a,b) => distance(a)-distance(b))[0] ?? null;
}

export function mirror(d) {
    d.rooms = d.rooms.map(({role,box:[x,z,w,h]}) => room(role,d.nx-w,z,d.nx-x,h));
    d.doors = d.doors.map(([axis,x,z]) => [axis,d.nx-x-(axis === "Z" ? 1 : 0),z]);
    d.entrances = d.entrances.map(([side,n]) => side === "Left" ? ["Right",n] : side === "Right" ? ["Left",n] : [side,d.nx-1-n]);
    if (d.stairs) d.stairs[0] = d.nx-2-d.stairs[0];
}

export function serialize(d) {
    return [
        "VXPLAN 1", "role " + d.role, "size " + d.nx + " " + d.nz, "bay " + d.bay, "height " + d.height,
        "floors " + d.floors, "roof " + d.roof, "windows " + d.windows, "age " + d.age, "seed " + d.seed,
        "palette " + d.palette.join(" "), ...(d.stairs ? ["stairs " + d.stairs.join(" ")] : []),
        ...d.rooms.map(r => "room " + r.role + " " + r.box.join(" ")),
        ...d.doors.map(e => "door " + e.join(" ")), ...d.entrances.map(e => "entrance " + e.join(" ")),
        "",
    ].join("\n");
}

export function parse(source) {
    const [magic,...lines] = source.split(/\r?\n/).map(l => l.trim()).filter(l => l && !l.startsWith("#"));
    if (magic !== "VXPLAN 1") throw Error("Open a VXPLAN design exported by Anchor Studio.");
    const d = { ...template("Workshop"), rooms: [], doors: [], entrances: [], stairs: null };
    for (const line of lines) {
        const [key,...args] = line.split(/\s+/);
        const [a,b] = args;
        switch (key) {
            case "size": [d.nx,d.nz] = args.map(Number); break;
            case "stairs": d.stairs = args.map(Number); break;
            case "palette": d.palette = args; break;
            case "room": d.rooms.push({ role:a, box:args.slice(1).map(Number) }); break;
            case "door": d.doors.push([a,...args.slice(1).map(Number)]); break;
            case "entrance": d.entrances.push([a,Number(b)]); break;
            case "role": case "roof": case "windows": d[key] = a; break;
            case "bay": case "height": case "floors": case "seed": case "age": d[key] = Number(a); break;
            default: throw Error("Unknown design setting: " + key);
        }
    }
    if (![d.nx,d.nz,d.floors].every(n => Number.isInteger(n) && n >= 1 && n <= 24)
        || !roles.includes(d.role) || d.palette.length !== 6
        || !["Flat","Gable","Hip","SteepHip","Lean","Glass"].includes(d.roof)
        || !["None","Panes","Slits","Glazed"].includes(d.windows)
        || ![d.bay,d.height,d.age,d.seed].every(Number.isFinite)
        || !d.rooms.every(r => roomTypes[r.role] && r.box.length === 4 && r.box.every(Number.isInteger))
        || !d.doors.every(([axis,x,z]) => ["X","Z"].includes(axis) && Number.isInteger(x) && Number.isInteger(z))
        || !d.entrances.every(([side,bay]) => ["Front","Back","Left","Right"].includes(side) && Number.isInteger(bay))
        || d.stairs && (d.stairs.length !== 2 || !d.stairs.every(Number.isInteger))) throw Error("Some design settings are unreadable.");
    return d;
}

export function decodeBlueprint(buffer) {
    const bytes = new Uint8Array(buffer), data = new DataView(buffer), decoder = new TextDecoder();
    if (decoder.decode(bytes.slice(0,8)) !== "VXBLUEPR" || data.getUint16(8,true) !== 1) throw Error("Blueprint response is unreadable.");
    const size = [10,12,14].map(offset => data.getUint16(offset,true));
    const count = data.getUint16(16,true);
    let at=18;
    const palette = Array.from({length:count}, () => {
        const length=bytes[at++];
        const name=decoder.decode(bytes.slice(at,at+length));
        at+=length;
        return name;
    });
    const runs = data.getUint32(at,true); at+=4;
    const cells = new Uint8Array(size.reduce((a,b)=>a*b,1));
    let cell=0;
    for (let i=0; i<runs; i++) {
        const id=bytes[at++];
        let n=0, shift=0, byte;
        do { byte=bytes[at++]; n|=(byte&127)<<shift; shift+=7; } while (byte&128);
        if (cell+n>cells.length || n<1 || id>=palette.length) throw Error("Blueprint response is incomplete.");
        cells.fill(id,cell,cell+n); cell+=n;
    }
    if (cell !== cells.length || at !== bytes.length-8) throw Error("Blueprint response is incomplete.");
    return { size, palette, cells };
}
