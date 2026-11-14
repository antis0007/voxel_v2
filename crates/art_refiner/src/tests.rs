use super::*;

fn options() -> Options {
    Options {
        tiles: "industry".into(), faces: vec![1], zoom: 1.2, dist: 1.0,
        tile_px: 16, cols: 4, origin: [0.3; 3], light: [-0.35, 0.45, 0.82],
        view: [0.35, -0.05, 0.94], seed: 0, out: PathBuf::new(), watch: false, labels: true,
    }
}

#[test]
fn industry_preserves_block_identity_beside_shared_kind() {
    let specs = tiles::resolve("concrete,steelplate,rustedsteel,hazardpaint,windowglass,stonebricks").unwrap();
    assert_eq!(specs.iter().map(|s| s.block).collect::<Vec<_>>()[..5], [192, 193, 194, 195, 196]);
    assert!(specs.iter().all(|s| s.kind == 12));
    assert_ne!(specs[5].block, specs[0].block);
    let (scene, _) = build_scene(&options()).unwrap();
    assert_eq!(scene.tiles[1].block, 193);
    assert_eq!(scene.tiles[4].rough, 0.10);
    assert_eq!(scene.tiles[4].refl, 0.55);
}

#[test]
fn captions_start_at_albedo_after_all_four_bands() {
    let (scene, names) = build_scene(&options()).unwrap();
    assert_eq!(names.len(), 8);
    assert_eq!(scene.size(), (64, 128));
    assert_eq!(label_origin(0, &scene), (2, 2));
    assert_eq!(label_origin(3, &scene), (50, 2));
    assert_eq!(label_origin(4, &scene), (2, 66));
    let harness = include_str!("../harness.wgsl");
    assert!(harness.contains("let height = rows * t_px * 4u;"));
    assert!(harness.contains("let mode = band % 4u;"));
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn gpu_refiner_compiles_current_world_shader_and_renders_industry() {
    let gpu = gpu::Gpu::from_adapter(engine_gpu::test_adapter()).unwrap();
    let pipeline = gpu.compile(&compose().unwrap()).unwrap();
    let (scene, _) = build_scene(&options()).unwrap();
    let pixels = gpu.render(&pipeline, &scene).unwrap();
    let (width, height) = scene.size();
    assert_eq!(pixels.len(), (width * height * 4) as usize);
    assert!(pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
    assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[..3] != [0, 0, 0]));
}
