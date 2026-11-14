//! Platform-independent compatibility ID, not an authentication credential.
//! Hash source/configuration/content in a deterministic path order. No target
//! paths, timestamps, compiler outputs, or host-specific metadata enter it.
use std::{path::{Path,PathBuf},io::Read};
pub fn emit(root:&Path) {
    fn files(path:&Path,out:&mut Vec<PathBuf>){
        for entry in std::fs::read_dir(path).expect("network build inputs"){
            let path=entry.expect("network build entry").path();
            if path.is_dir(){
                if !matches!(path.file_name().and_then(|n|n.to_str()),Some("target"|"source"|".git")){files(&path,out);}
            }else if matches!(path.extension().and_then(|e|e.to_str()),Some("rs"|"toml"|"lock"|"ron"|"wgsl"|"json"|"png"|"glb"|"gltf"|"ttf"|"otf"|"wav"|"ogg")){out.push(path);}
        }
    }
    let mut paths=Vec::new();files(&root.join("crates"),&mut paths);files(&root.join("assets"),&mut paths);
    paths.extend([root.join("Cargo.toml"),root.join("Cargo.lock")]);
    paths.sort_by_key(|p|p.strip_prefix(root).unwrap().to_string_lossy().replace('\\',"/"));
    let mut hashes=[0xcbf29ce484222325u64,0x84222325cbf29ce4,0x9e3779b97f4a7c15,0x517cc1b727220a95];
    let mut absorb=|bytes:&[u8]|{for &b in bytes{for (i,hash) in hashes.iter_mut().enumerate(){*hash=(*hash^u64::from(b)).wrapping_mul(0x100000001b3+(i as u64)*2);}}};
    let mut buffer=[0u8;65536];
    for path in paths {
        println!("cargo:rerun-if-changed={}",path.display());
        let relative=path.strip_prefix(root).unwrap().to_string_lossy().replace('\\',"/");
        absorb(&(relative.len() as u64).to_le_bytes());absorb(relative.as_bytes());
        let mut file=std::fs::File::open(&path).expect("network build input");
        absorb(&file.metadata().unwrap().len().to_le_bytes());
        loop{let n=file.read(&mut buffer).expect("network build read");if n==0{break;}absorb(&buffer[..n]);}
    }
    let id=hashes.iter().map(|h|format!("{h:016x}")).collect::<String>();
    println!("cargo:rustc-env=VOXEL_NETWORK_BUILD_ID={id}");
    println!("cargo:warning=Network compatibility build={id}");
}
