use super::*;

#[test]
fn a_cube_has_12_triangles_and_its_bounds() {
    let cube = Solid::cube(2.0, 4.0, 1.0);
    assert_eq!(format!("{cube:?}"), "12 triangles, bounds -1 -2 -0.5 to 1 2 0.5");
    assert_eq!(format!("{:?}", Solid::empty()), "empty");
    assert_eq!(format!("{:?}", Solid::default().clone()), "empty");
}

#[test]
fn booleans_and_moves_change_the_solid() {
    let a = Solid::cube(2.0, 2.0, 2.0);
    let b = a.translate([1.0, 0.0, 0.0]);
    assert_eq!(a.union(&b).bounds(), [[-1.0, -1.0, -1.0], [2.0, 1.0, 1.0]]);
    assert_eq!(a.difference(&b).bounds(), [[-1.0, -1.0, -1.0], [0.0, 1.0, 1.0]]);
    assert_eq!(a.intersection(&b).bounds(), [[0.0, -1.0, -1.0], [1.0, 1.0, 1.0]]);
    let tall = a.scale([1.0, 1.0, 3.0]).rotate([90.0, 0.0, 0.0]);
    let [low, high] = tall.bounds();
    assert!((low[1] + 3.0).abs() < 1e-9 && (high[2] - 1.0).abs() < 1e-9, "{low:?} {high:?}");
    assert!(Solid::sphere(1.0, 16).triangles() > 0 && Solid::cylinder(2.0, 1.0, 0.5, 12).triangles() > 0);
    let bad = Solid::cube(-1.0, 1.0, 1.0);
    assert_eq!(bad.error(), Some("the construction is not valid"));
    assert_eq!(format!("{bad:?}"), "error: the construction is not valid");
    assert_eq!(a.error(), None);
}

#[test]
fn the_mesh_keeps_the_part_of_each_triangle() {
    let (a, b) = (Solid::cube(2.0, 2.0, 2.0).as_part(), Solid::cube(1.0, 1.0, 1.0).translate([1.5, 0.0, 0.0]).as_part());
    assert_ne!(a.part().unwrap(), b.part().unwrap());
    let mesh = a.union(&b).mesh();
    assert_eq!(mesh.positions.len(), mesh.triangles() * 3);
    assert_eq!(mesh.normals.len(), mesh.positions.len());
    assert_eq!(mesh.uvs.len(), mesh.positions.len());
    assert!(mesh.parts.contains(&0) && mesh.parts.contains(&1), "{:?}", mesh.parts);
    assert!(mesh.parts.iter().all(|p| *p < 2));
    // The small box touches the large box only on one face, so its own triangles are on x > 1.
    for (tri, &part) in mesh.parts.iter().enumerate() {
        let x = mesh.positions[tri * 3..tri * 3 + 3].iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        assert!(part == 0 || x > 1.0, "{part} {x}");
    }
    for n in &mesh.normals {
        assert!(((n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() - 1.0).abs() < 1e-5);
    }
}

#[test]
fn the_thumbnail_draws_the_solid_on_a_clear_background() {
    let mesh = Solid::cube(1.0, 1.0, 1.0).mesh();
    let rgba = picture(&mesh, THUMBNAIL);
    let alpha = |x: usize, y: usize| rgba[(y * THUMBNAIL + x) * 4 + 3];
    assert_eq!(alpha(0, 0), 0);
    assert_eq!(alpha(THUMBNAIL / 2, THUMBNAIL / 2), 255);
    let shades: std::collections::BTreeSet<[u8; 3]> =
        rgba.chunks_exact(4).filter(|p| p[3] == 255).map(|p| [p[0], p[1], p[2]]).collect();
    assert_eq!(shades.len(), 3, "the three faces to the eye have their own shade");
    assert!(picture(&Mesh::default(), 8).iter().all(|&b| b == 0));
    let mut ui = Ui::new(1, "");
    Solid::cube(1.0, 1.0, 1.0).show(&mut ui);
    assert!(ui.tree().starts_with("image\tSolid\t192\t192\t"), "{}", ui.tree());
}

#[test]
fn a_seed_gives_the_same_values_and_children_apart() {
    let seed = Seed(7);
    let take = |s: Seed| {
        let mut r = s.random();
        [r.next_u64(), r.next_u64()]
    };
    assert_eq!(take(seed), take(Seed(7)));
    assert_ne!(take(seed.child("barrel")), take(seed.child("grip")));
    assert_eq!(seed.child("barrel"), Seed(7).child("barrel"));
    // The first value of SplitMix64 for the seed 0, from the reference generator.
    assert_eq!(Seed(0).random().next_u64(), 0xe220_a839_7b1d_cdaf);
    let mut r = seed.random();
    for _ in 0..1000 {
        let (u, i) = (r.range(2.0, 3.0), r.index(5));
        assert!((2.0..3.0).contains(&u) && i < 5);
    }
    assert_eq!(r.index(0), 0);
    let mut counts = [0; 3];
    for _ in 0..3000 {
        counts[r.pick(&[1.0, 0.0, 2.0])] += 1;
    }
    assert!(counts[1] == 0 && counts[0] > 800 && counts[2] > 1700, "{counts:?}");
    assert_eq!(r.pick(&[0.0, -1.0]), 0);
}

#[test]
fn the_same_seed_gives_the_same_mesh() {
    let model = |seed: Seed| {
        let mut r = seed.random();
        (0..5).fold(Solid::empty(), |sum, i| {
            let size = [r.range(0.5, 2.0), r.range(0.5, 2.0), r.range(0.5, 2.0)];
            sum.union(&Solid::cube(size[0], size[1], size[2]).translate([i as f64, 0.0, size[2] / 2.0]))
        })
    };
    let hash = model(Seed(42)).mesh().hash();
    // The MSVC build and the Patina toolchain (Zig, gnu target) gave this hash on 2026-11-19.
    assert_eq!(hash, 0xe03e_96d6_7c36_eb63);
    assert_eq!(hash, model(Seed(42)).mesh().hash());
    assert_ne!(hash, model(Seed(43)).mesh().hash());
}

#[test]
fn sockets_keep_one_frame_for_each_name() {
    let mut sockets = Sockets::default();
    assert_eq!(format!("{sockets:?}"), "0 sockets");
    sockets.set("muzzle", Frame::at([0.0, 0.0, 10.0]));
    sockets.set("grip", Frame::default());
    sockets.set("muzzle", Frame::at([0.0, 0.0, 12.0]));
    assert_eq!(format!("{sockets:?}"), "2 sockets: muzzle, grip");
    assert_eq!(sockets.get("muzzle").map(|f| f.origin), Some([0.0, 0.0, 12.0]));
    assert_eq!(sockets.get("stock"), None);
}
