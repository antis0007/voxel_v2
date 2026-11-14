param(
    [Parameter(Mandatory)][string]$Candidate,
    [Parameter(Mandatory)][string]$Output,
    [string]$Reference = 'target/catchment-usgs-canyon.f32',
    [string]$ReferenceLabel = 'Little Wild Horse Canyon · USGS 3DEP',
    [string]$CandidateLabel = 'AXIOM',
    [string]$CandidateSectionFrom4m,
    [ValidateSet(2,4)][int]$CandidateSpacing = 4
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$assembly = [Drawing.Bitmap].Assembly.Location
$graphicsAssembly = [Drawing.Color].Assembly.Location
$drawingRefs = @(Get-ChildItem -LiteralPath (Split-Path $assembly) -Filter 'System.Private.Windows.*.dll' | ForEach-Object FullName)
Add-Type -ReferencedAssemblies (@($assembly, $graphicsAssembly, 'System.Runtime', 'System.IO.FileSystem', 'System.ComponentModel.Primitives', 'System.ComponentModel.TypeConverter', 'System.Runtime.InteropServices') + $drawingRefs) -TypeDefinition @'
using System;
using System.IO;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public static class CanyonReview {
    const int Span=4000, W=740, H=500, Step=8;
    const double Yaw=0.43, Pitch=0.56, Scale=0.127;
    struct Vertex { public double x,y,d,h; }
    sealed class Tile {
        public readonly int N, Spacing;
        public readonly float[] Z;
        public readonly double Min;
        public Tile(string path, string soil, int spacing) {
            Spacing=spacing; N=Span/spacing; Z=Read(path);
            if(soil!=null) {
                float[] s=Read(soil);
                for(int i=0;i<Z.Length;i++) Z[i]+=s[i];
            }
            double lo=double.PositiveInfinity;
            foreach(float v in Z) {
                if(float.IsNaN(v)||float.IsInfinity(v)) throw new ArgumentException("Nonfinite elevation");
                lo=Math.Min(lo,v);
            }
            Min=lo;
        }
        float[] Read(string path) {
            byte[] b=File.ReadAllBytes(path);
            if(b.Length!=4*N*N) throw new ArgumentException("Expected 4 km float32 elevation grid: "+path);
            var z=new float[N*N]; Buffer.BlockCopy(b,0,z,0,b.Length); return z;
        }
        public double At(double x,double y) {
            double s=Math.Max(0,Math.Min(N-1,x/Spacing-0.5));
            double t=Math.Max(0,Math.Min(N-1,y/Spacing-0.5));
            int i=(int)s,j=(int)t,ii=Math.Min(i+1,N-1),jj=Math.Min(j+1,N-1);
            double u=s-i,v=t-j;
            return (1-v)*((1-u)*Z[j*N+i]+u*Z[j*N+ii])+v*((1-u)*Z[jj*N+i]+u*Z[jj*N+ii]);
        }
    }
    struct Cut {
        public bool AcrossX;
        public int Along, Centre;
        public double Depth;
        public double At(Tile t,int s) {
            return AcrossX?t.At(Centre+s,Along):t.At(Along,Centre+s);
        }
        public Vertex Project(Tile t,int s) {
            return AcrossX?CanyonReview.Project(Centre+s,Along,At(t,s)-t.Min)
                          :CanyonReview.Project(Along,Centre+s,At(t,s)-t.Min);
        }
        public override string ToString() {
            return AcrossX?String.Format("along X at x={0}, y={1} m",Centre,Along)
                          :String.Format("along Y at x={0}, y={1} m",Along,Centre);
        }
    }
    // Select each tile's strongest interior depression bounded by two
    // shoulders at a fixed metric offset. This is a diagnostic transect,
    // not a claim that the two cross sections are corresponding locations.
    static Cut FindCut(Tile t) {
        var best=new Cut { Centre=2000, Along=2000, AcrossX=true };
        for(int a=0;a<2;a++) for(int x=640;x<=3360;x+=32) for(int y=640;y<=3360;y+=32) {
            var c=new Cut { Centre=x,Along=y,AcrossX=a==0 };
            double l=Math.Min(c.At(t,-480),c.At(t,-600));
            double r=Math.Min(c.At(t,480),c.At(t,600));
            double floor=c.At(t,0);
            c.Depth=Math.Min(l,r)-floor;
            if(c.Depth>best.Depth) best=c;
        }
        return best;
    }
    static Vertex Project(double x,double y,double h) {
        double u=x-2000,v=y-2000;
        double s=u*Math.Cos(Yaw)+v*Math.Sin(Yaw), t=-u*Math.Sin(Yaw)+v*Math.Cos(Yaw);
        return new Vertex { x=W/2.0+s*Scale, y=H/2.0+26+(t*Math.Sin(Pitch)-h*Math.Cos(Pitch))*Scale,
                            d=t*Math.Cos(Pitch)+h*Math.Sin(Pitch),h=h };
    }
    static double Cross(double x,double y,double u,double v) {return x*v-y*u;}
    static void Tri(Vertex a,Vertex b,Vertex c,int colour,double[] depth,int[] pixels) {
        double q=Cross(b.x-a.x,b.y-a.y,c.x-a.x,c.y-a.y);
        if(Math.Abs(q)<1e-9) return;
        int x0=Math.Max(0,(int)Math.Floor(Math.Min(a.x,Math.Min(b.x,c.x))));
        int x1=Math.Min(W-1,(int)Math.Ceiling(Math.Max(a.x,Math.Max(b.x,c.x))));
        int y0=Math.Max(0,(int)Math.Floor(Math.Min(a.y,Math.Min(b.y,c.y))));
        int y1=Math.Min(H-1,(int)Math.Ceiling(Math.Max(a.y,Math.Max(b.y,c.y))));
        for(int y=y0;y<=y1;y++) for(int x=x0;x<=x1;x++) {
            double u=Cross(x+0.5-a.x,y+0.5-a.y,c.x-a.x,c.y-a.y)/q;
            double v=Cross(b.x-a.x,b.y-a.y,x+0.5-a.x,y+0.5-a.y)/q;
            if(u<0 || v<0 || u+v>1) continue;
            double d=a.d+u*(b.d-a.d)+v*(c.d-a.d);
            int i=y*W+x;
            if(d>depth[i]) {depth[i]=d;pixels[i]=colour;}
        }
    }
    static Bitmap Render(Tile t,Cut cut) {
        int n=Span/Step+1;
        var pts=new Vertex[n*n];
        var depth=new double[W*H]; var px=new int[W*H];
        for(int i=0;i<px.Length;i++) {depth[i]=double.NegativeInfinity;px[i]=unchecked((int)0xff141a1e);}
        for(int z=0;z<n;z++) for(int x=0;x<n;x++) pts[z*n+x]=Project(x*Step,z*Step,t.At(x*Step,z*Step)-t.Min);
        for(int z=0;z<n-1;z++) for(int x=0;x<n-1;x++) {
            int i=z*n+x; var a=pts[i];var b=pts[i+1];var c=pts[i+n];var d=pts[i+n+1];
            double gx=(b.h+d.h-a.h-c.h)/(2*Step), gy=(c.h+d.h-a.h-b.h)/(2*Step);
            double light=Math.Max(0,(0.69+0.62*gx+0.37*gy)/Math.Sqrt(1+gx*gx+gy*gy));
            int shade=(int)(57+181*Math.Min(1,light));
            int color=unchecked((int)0xff000000)|(shade<<16)|(shade<<8)|shade;
            Tri(a,b,c,color,depth,px);Tri(b,d,c,color,depth,px);
        }
        // Paint the section on the visible surface only, so the line does
        // not appear to jump across a hidden face.
        for(int s=-600;s<=600;s+=2) {
            Vertex p=cut.Project(t,s);
            int x=(int)p.x,y=(int)p.y;
            for(int j=-1;j<=1;j++) for(int k=-1;k<=1;k++) {
                int xx=x+k,yy=y+j;
                if(xx<0||yy<0||xx>=W||yy>=H) continue;
                int i=yy*W+xx;
                if(p.d>=depth[i]-18) px[i]=unchecked((int)0xffffb85d);
            }
        }
        var bmp=new Bitmap(W,H,PixelFormat.Format32bppArgb);
        var bdata=bmp.LockBits(new Rectangle(0,0,W,H),ImageLockMode.WriteOnly,bmp.PixelFormat);
        Marshal.Copy(px,0,bdata.Scan0,px.Length);
        bmp.UnlockBits(bdata);
        return bmp;
    }
    static double Range(Tile t,Cut c) {
        double lo=double.PositiveInfinity,hi=double.NegativeInfinity;
        for(int s=-600;s<=600;s+=4) {double h=c.At(t,s);lo=Math.Min(lo,h);hi=Math.Max(hi,h);}
        return hi-lo;
    }
    static int SectionFloor(Tile t,Cut c) {
        int floor=0;
        for(int s=-40;s<=40;s+=2) {
            if(c.At(t,s)<c.At(t,floor)) floor=s;
        }
        return floor;
    }
    static double RiseCrossing(Tile t,Cut c,int floor,int direction,double rise) {
        double threshold=c.At(t,floor)+rise;
        int previous=floor;
        for(int s=floor+direction;s>=-600&&s<=600;s+=direction) {
            double h=c.At(t,s);
            if(h>=threshold) {
                double low=c.At(t,previous);
                return previous+direction*(threshold-low)/(h-low);
            }
            previous=s;
        }
        return double.NaN;
    }
    static string ValleyWidths(Tile t,Cut c) {
        int floor=SectionFloor(t,c);
        string[] widths=new string[2];
        int[] rises={20,60};
        for(int i=0;i<rises.Length;i++) {
            double left=RiseCrossing(t,c,floor,-1,rises[i]);
            double right=RiseCrossing(t,c,floor,1,rises[i]);
            widths[i]=double.IsNaN(right-left)?"—":String.Format("{0:F0} m",right-left);
        }
        return String.Format("Section width · {0} at +20 m · {1} at +60 m",widths[0],widths[1]);
    }
    // Subtract the mean of opposing samples to remove local planar tilt.
    // The quantiles distinguish pervasive texture from a few sharp cliffs.
    static string SurfaceResidual(Tile t,int offset,bool upland=false) {
        int side=1+(Span-2*offset)/16;
        var residuals=new double[side*side];
        double uplandFloor=double.NegativeInfinity;
        if(upland) {
            var elevations=new double[side*side];
            int sample=0;
            for(int y=offset;y<=Span-offset;y+=16)
                for(int x=offset;x<=Span-offset;x+=16) elevations[sample++]=t.At(x,y);
            Array.Sort(elevations);
            uplandFloor=elevations[(int)((sample-1)*0.65)];
        }
        int i=0;
        for(int y=offset;y<=Span-offset;y+=16) {
            for(int x=offset;x<=Span-offset;x+=16) {
                double h=t.At(x,y);
                if(h<uplandFloor) continue;
                double west=t.At(x-offset,y), east=t.At(x+offset,y);
                double south=t.At(x,y-offset), north=t.At(x,y+offset);
                if(upland && Math.Pow(east-west,2)+Math.Pow(north-south,2)>Math.Pow(0.36*offset,2)) continue;
                residuals[i++]=Math.Abs(h-(west+east+south+north)/4);
            }
        }
        if(i==0) return "no matching samples";
        Array.Sort(residuals,0,i);
        return String.Format("{0} m offset: median {1:F2} m, 90th percentile {2:F2} m",
            offset,residuals[i/2],residuals[(int)((i-1)*0.9)]);
    }
    static void Profile(Graphics g,Tile t,Cut c,int left,double span,Font body) {
        const int top=601,width=660,height=195;
        var points=new PointF[301];
        double min=double.PositiveInfinity;
        for(int i=0;i<points.Length;i++) min=Math.Min(min,c.At(t,4*i-600));
        for(int i=0;i<points.Length;i++) points[i]=new PointF(left+(float)(i*width/300.0),
            top+height-(float)((c.At(t,4*i-600)-min)*height/span));
        using(var grid=new Pen(Color.FromArgb(53,62,68)))
        using(var line=new Pen(Color.FromArgb(255,184,93),2f)) {
            for(int y=0;y<=4;y++) g.DrawLine(grid,left,top+y*height/4,left+width,top+y*height/4);
            for(int x=0;x<=4;x++) g.DrawLine(grid,left+x*width/4,top,left+x*width/4,top+height);
            g.DrawLines(line,points);
        }
        g.DrawString("0 m",body,Brushes.LightGray,left,top+height+5);
        g.DrawString("1,200 m",body,Brushes.LightGray,left+width-55,top+height+5);
        g.DrawString(String.Format("{0:F0} m rise · section datum {1:F0} m",Range(t,c),min),
            body,Brushes.LightGray,left,top-23);
        g.DrawString(ValleyWidths(t,c),body,Brushes.LightGray,left,top+height+26);
    }
    public static string Save(string bedrock,string sediment,int spacing,string reference,string anchor,
                              string candidateLabel,string referenceLabel,string output) {
        var a=new Tile(bedrock,sediment,spacing);var b=new Tile(reference,null,4);
        var sectionTile=String.IsNullOrEmpty(anchor)?a:new Tile(
            Path.Combine(anchor,"canyon-bedrock.f32"),Path.Combine(anchor,"canyon-sediment.f32"),4);
        Cut ca=FindCut(sectionTile), cb=FindCut(b);
        double span=Math.Max(50,Math.Ceiling(Math.Max(Range(a,ca),Range(b,cb))/50)*50);
        using(var im=new Bitmap(1516,879))
        using(var g=Graphics.FromImage(im))
        using(var title=new Font("Segoe UI",20,FontStyle.Bold))
        using(var body=new Font("Segoe UI",12))
        using(var va=Render(a,ca))
        using(var vb=Render(b,cb)) {
            g.Clear(Color.FromArgb(20,26,30));
            g.DrawString("Canyon geometry",title,Brushes.White,16,8);
            g.DrawString("4 km windows · matched camera and scale · true-height meshes",body,Brushes.LightGray,17,49);
            g.DrawImageUnscaled(va,12,89);g.DrawImageUnscaled(vb,766,89);
            g.DrawString(candidateLabel,body,Brushes.White,17,80);
            g.DrawString(referenceLabel,body,Brushes.White,771,80);
            Profile(g,a,ca,40,span,body);Profile(g,b,cb,794,span,body);
            g.DrawString(String.Format("Orange traces a 1.2 km valley section · profiles share a 0–{0:F0} m vertical scale",span),
                body,Brushes.LightGray,17,854);
            im.Save(output,ImageFormat.Png);
        }
        string report=String.Format("Candidate section relief={0:F1} m, reference section relief={1:F1} m\nCandidate {2}\nReference {3}"
            +"\nAbsolute elevation residual after removing local tilt\nCandidate {4}; {5}\nReference {6}; {7}",
            Range(a,ca),Range(b,cb),ValleyWidths(a,ca),ValleyWidths(b,cb),
            SurfaceResidual(a,24),SurfaceResidual(a,96),SurfaceResidual(b,24),SurfaceResidual(b,96))
            +String.Format("\nCandidate section {0}\nReference section {1}",ca,cb);
        string initialPath=Path.Combine(Path.GetDirectoryName(bedrock),"canyon-initial.f32");
        if(File.Exists(initialPath)) {
            var initial=new Tile(initialPath,null,spacing);
            report+=String.Format("\nInitial relief at same section {0:F1} m\nInitial {1}\nInitial {2}; {3}",
                Range(initial,ca),ValleyWidths(initial,ca),SurfaceResidual(initial,24),SurfaceResidual(initial,96));
        }
        report+=String.Format("\nGentle uplands (highest 35% of elevations, grade at most 18%)\nCandidate {0}\nReference {1}",
            SurfaceResidual(a,24,true),SurfaceResidual(b,24,true));
        return report;
    }
}
'@
$candidateRoot=(Resolve-Path -LiteralPath $Candidate).Path
$referencePath=(Resolve-Path -LiteralPath $Reference).Path
$anchorPath=if($CandidateSectionFrom4m){(Resolve-Path -LiteralPath $CandidateSectionFrom4m).Path}else{''}
$outputPath=[IO.Path]::GetFullPath($Output)
if(Test-Path -LiteralPath $outputPath){throw "Output already exists: $outputPath"}
[CanyonReview]::Save((Join-Path $candidateRoot 'canyon-bedrock.f32'),
    (Join-Path $candidateRoot 'canyon-sediment.f32'),$CandidateSpacing,$referencePath,$anchorPath,
    $CandidateLabel,$ReferenceLabel,$outputPath)
