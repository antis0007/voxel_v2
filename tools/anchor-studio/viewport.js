import { colors } from "./design.js";

const faces = [
    { n:[1,0,0], light:.77, q:[[1,0,0],[1,1,0],[1,1,1],[1,0,1]] },
    { n:[-1,0,0], light:.64, q:[[0,0,1],[0,1,1],[0,1,0],[0,0,0]] },
    { n:[0,1,0], light:1.0, q:[[0,1,1],[1,1,1],[1,1,0],[0,1,0]] },
    { n:[0,-1,0], light:.48, q:[[0,0,0],[1,0,0],[1,0,1],[0,0,1]] },
    { n:[0,0,1], light:.88, q:[[1,0,1],[1,1,1],[0,1,1],[0,0,1]] },
    { n:[0,0,-1], light:.68, q:[[0,0,0],[0,1,0],[1,1,0],[1,0,0]] },
];
const triangles = [0,1,2,0,2,3];
const hex = name => {
    const n = parseInt((colors[name] ?? "#849297").slice(1),16);
    return [(n>>16&255)/255,(n>>8&255)/255,(n&255)/255];
};

function shader(gl, type, source) {
    const result = gl.createShader(type);
    gl.shaderSource(result,source);
    gl.compileShader(result);
    if (!gl.getShaderParameter(result,gl.COMPILE_STATUS)) throw Error(gl.getShaderInfoLog(result));
    return result;
}

function program(gl) {
    const result = gl.createProgram();
    gl.attachShader(result, shader(gl,gl.VERTEX_SHADER,`
        attribute vec3 position;
        attribute vec3 color;
        uniform vec3 focus, right, up, forward;
        uniform vec3 zoom;
        varying vec3 light;
        void main() {
            vec3 p = position - focus;
            gl_Position = vec4(dot(p, right) * zoom.x, dot(p, up) * zoom.y, -dot(p, forward) * zoom.z, 1.0);
            light = color;
        }`));
    gl.attachShader(result, shader(gl,gl.FRAGMENT_SHADER,`
        precision mediump float;
        varying vec3 light;
        void main() { gl_FragColor = vec4(light, 1.0); }
    `));
    gl.linkProgram(result);
    if (!gl.getProgramParameter(result,gl.LINK_STATUS)) throw Error(gl.getProgramInfoLog(result));
    return result;
}

export class Viewport {
    constructor(canvas) {
        this.canvas = canvas;
        const gl = canvas.getContext("webgl", {alpha:true,antialias:true});
        if (!gl) throw Error("WebGL is unavailable. The floor-plan editor and blueprint export still work.");
        this.gl = gl;
        this.program = program(gl);
        this.mesh = gl.createBuffer();
        this.grid = gl.createBuffer();
        this.count = 0;
        this.gridCount = 0;
        this.yaw = .64;
        this.pitch = .48;
        this.magnify = 1;
        this.target = [0,0,0];
        this.bounds = [18,12,16];
        this.names = Object.fromEntries(["focus","right","up","forward","zoom"]
            .map(name => [name,gl.getUniformLocation(this.program,name)]));
        this.position = gl.getAttribLocation(this.program,"position");
        this.color = gl.getAttribLocation(this.program,"color");
        gl.enable(gl.DEPTH_TEST);
        gl.clearColor(0,0,0,0);
        new ResizeObserver(() => this.draw()).observe(canvas);
        canvas.addEventListener("contextmenu",e => e.preventDefault());
        canvas.addEventListener("wheel",e => {
            e.preventDefault();
            this.magnify = Math.max(.25,Math.min(4,this.magnify*Math.exp(-e.deltaY*.0013)));
            this.draw();
        },{passive:false});
        this.makeGrid();
    }

    basis() {
        const c = Math.cos(this.yaw), s = Math.sin(this.yaw);
        const h = Math.sin(this.pitch), v = Math.cos(this.pitch);
        return { right:[c,0,-s], up:[-s*h,v,-c*h], forward:[s*v,h,c*v] };
    }

    fit() {
        this.target = this.bounds.map(n=>n/2);
        this.yaw = .64;
        this.pitch = .48;
        this.magnify = 1;
        this.draw();
    }

    model(model, cut, refit) {
        const [sx,sy,sz] = model.size;
        this.bounds = model.size;
        const gl = this.gl;
        const top = Math.min(sy,cut);
        const index = (x,y,z) => x + sx*(z+sz*y);
        const air = model.palette.indexOf("Air");
        const rgb = model.palette.map(hex);
        const vertices = [];
        for (let y=0; y<top; y++) for (let z=0; z<sz; z++) for (let x=0; x<sx; x++) {
            const i = model.cells[index(x,y,z)];
            if (i === air) continue;
            for (const {n:[dx,dy,dz],light,q} of faces) {
                const a=x+dx,b=y+dy,c=z+dz;
                if (a>=0 && a<sx && b>=0 && b<top && c>=0 && c<sz && model.cells[index(a,b,c)]!==air) continue;
                const [r,g,blue] = rgb[i];
                for (const vertex of triangles) {
                    const [u,v,w] = q[vertex];
                    vertices.push(x+u,y+v,z+w,r*light,g*light,blue*light);
                }
            }
        }
        gl.bindBuffer(gl.ARRAY_BUFFER,this.mesh);
        gl.bufferData(gl.ARRAY_BUFFER,new Float32Array(vertices),gl.STATIC_DRAW);
        this.count = vertices.length/6;
        this.makeGrid();
        if (refit) this.fit();
        else this.draw();
    }

    makeGrid() {
        const gl = this.gl, [w,,d] = this.bounds;
        const step = Math.max(1,Math.ceil(Math.max(w,d)/30));
        const vertices=[];
        const p = (x,z)=>vertices.push(x,-.03,z,.19,.25,.28);
        for(let x=-step*4;x<=w+step*4;x+=step) {p(x,-step*4);p(x,d+step*4);}
        for(let z=-step*4;z<=d+step*4;z+=step) {p(-step*4,z);p(w+step*4,z);}
        gl.bindBuffer(gl.ARRAY_BUFFER,this.grid);
        gl.bufferData(gl.ARRAY_BUFFER,new Float32Array(vertices),gl.STATIC_DRAW);
        this.gridCount=vertices.length/6;
    }

    span() {
        const r=this.canvas.getBoundingClientRect();
        const max = Math.hypot(...this.bounds);
        const height = max*1.03/this.magnify;
        const width = height*r.width/Math.max(1,r.height);
        return [width,height,max];
    }

    orbit(dx,dy,pan) {
        if (pan) {
            const {right,up} = this.basis();
            const [width,height] = this.span(), rect=this.canvas.getBoundingClientRect();
            this.target = this.target.map((p,i)=>p-right[i]*dx*width/rect.width+up[i]*dy*height/rect.height);
        } else {
            this.yaw -= dx*.008;
            this.pitch = Math.max(.16,Math.min(1.48,this.pitch+dy*.007));
        }
        this.draw();
    }

    planPoint(event, bay, scale) {
        const r=this.canvas.getBoundingClientRect();
        const {right,up,forward}=this.basis();
        const [width,height]=this.span();
        const x=(event.clientX-r.left-r.width/2)*width/r.width;
        const y=(r.top+r.height/2-event.clientY)*height/r.height;
        const p=this.target.map((v,i)=>v+x*right[i]+y*up[i]);
        const t=-p[1]/forward[1];
        return [(p[0]+t*forward[0])*scale/bay-1/bay,(p[2]+t*forward[2])*scale/bay-1/bay];
    }

    draw() {
        const canvas=this.canvas, gl=this.gl;
        const dpi=Math.min(2,window.devicePixelRatio||1);
        const w=Math.round(canvas.clientWidth*dpi),h=Math.round(canvas.clientHeight*dpi);
        if (!w||!h) return;
        if (canvas.width!==w||canvas.height!==h) {canvas.width=w; canvas.height=h;}
        gl.viewport(0,0,w,h);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
        gl.useProgram(this.program);
        const [width,height,max]=this.span();
        const {right,up,forward}=this.basis();
        gl.uniform3fv(this.names.focus,this.target);
        gl.uniform3fv(this.names.right,right);
        gl.uniform3fv(this.names.up,up);
        gl.uniform3fv(this.names.forward,forward);
        gl.uniform3f(this.names.zoom,2/width,2/height,1/Math.max(1,max*4));
        for (const [buffer,mode,count] of [[this.grid,gl.LINES,this.gridCount],[this.mesh,gl.TRIANGLES,this.count]]) {
            gl.bindBuffer(gl.ARRAY_BUFFER,buffer);
            gl.enableVertexAttribArray(this.position);
            gl.vertexAttribPointer(this.position,3,gl.FLOAT,false,24,0);
            gl.enableVertexAttribArray(this.color);
            gl.vertexAttribPointer(this.color,3,gl.FLOAT,false,24,12);
            gl.drawArrays(mode,0,count);
        }
    }
}
