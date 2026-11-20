use super::*;
use patina_sdk::Param;

#[test]
fn a_cube_has_12_triangles_and_its_bounds() {
    let cube = Solid::cube(2.0, 4.0, 1.0);
    assert_eq!(format!("{cube:?}"), "12 triangles, bounds -1 -2 -0.5 to 1 2 0.5");
    assert_eq!(format!("{:?}", Solid::empty()), "empty");
    assert_eq!(format!("{:?}", Solid::default().clone()), "empty");
    // A copy has a new serial, so a view sees the copy of each run as a new value.
    assert_ne!(cube.serial(), cube.clone().serial());
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
        rgba.as_chunks::<4>().0.iter().filter(|p| p[3] == 255).map(|p| [p[0], p[1], p[2]]).collect();
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
    assert_eq!(hash, 0x5619_ff62_71be_e9db);
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

#[test]
fn sketches_make_solids() {
    let rect = Sketch::rectangle(2.0, 1.0);
    assert_eq!(format!("{rect:?}"), "4 corners, area 2, bounds -1 -0.5 to 1 0.5");
    assert_eq!(format!("{:?}", Sketch::default().clone()), "empty");
    assert_eq!(rect.translate([1.0, 2.0]).bounds(), [[0.0, 1.5], [2.0, 2.5]]);
    // The points turn clockwise, and the fill rule fills them too.
    assert_eq!(Sketch::polygon(&[[0.0, 0.0], [0.0, 2.0], [2.0, 0.0]]).area(), 2.0);
    assert_eq!(Sketch::circle(1.0, 6).corners(), 6);
    assert_eq!(Sketch::circle(1.0, 100_000).corners(), MAX_SEGMENTS as usize);
    // Clipper2 throws on a coordinate out of its range. The sketch is empty and keeps the error.
    let far = Sketch::polygon(&[[0.0, 0.0], [1e300, 0.0], [0.0, 1e300]]);
    assert_eq!(format!("{:?}", far.translate([1.0, 0.0]).clone()), "error: the construction is not valid");
    assert_eq!(far.corners(), 0);
    assert_eq!(Solid::extrude(&far, 1.0, 0.0, [1.0; 2], 0).error(), Some("the construction is not valid"));
    assert_eq!(Solid::revolve(&far, 360.0, 8).error(), Some("the construction is not valid"));
    assert_eq!(far.clone().checked().unwrap_err(), "the kernel gave an error: the construction is not valid");
    assert_eq!(rect.clone().checked().unwrap().error(), None);
    let block = Solid::extrude(&rect, 3.0, 0.0, [1.0; 2], 0);
    assert_eq!(block.bounds(), [[-1.0, -0.5, 0.0], [1.0, 0.5, 3.0]]);
    let point = Solid::extrude(&rect, 1.0, 0.0, [0.0; 2], 0).checked().unwrap();
    assert_eq!(point.bounds()[1][2], 1.0);
    let ring = Solid::revolve(&Sketch::rectangle(1.0, 1.0).translate([1.5, 0.0]), 360.0, 8).checked().unwrap();
    let [low, high] = ring.bounds();
    let near = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-9);
    assert!(near(low, [-2.0, -2.0, -0.5]) && near(high, [2.0, 2.0, 0.5]), "{low:?} {high:?}");
    let mut ui = Ui::new(1, "");
    rect.show(&mut ui);
    assert!(ui.tree().starts_with("image\tSketch\t192\t192\t"), "{}", ui.tree());
}

#[test]
fn a_projection_gives_the_outline_and_its_holes() {
    let cube = Solid::cube(2.0, 2.0, 2.0);
    let top = cube.project();
    assert_eq!(top.area(), 4.0);
    assert_eq!(top.outlines().iter().map(Vec::len).collect::<Vec<_>>(), [4]);
    let frame = cube.difference(&Solid::cube(1.0, 1.0, 3.0));
    let outlines = frame.project().outlines();
    assert_eq!(outlines.len(), 2, "{outlines:?}");
    assert_eq!(frame.project().area(), 3.0);
    // The side view: Y turns to Z, so the long Z side of the block shows on Y.
    let side = Solid::cube(1.0, 1.0, 4.0).rotate([-90.0, 0.0, 0.0]).project();
    let [[_, y0], [_, y1]] = side.bounds();
    assert!((y1 - y0 - 4.0).abs() < 1e-9, "{side:?}");
    assert_eq!(format!("{:?}", Solid::empty().project()), "empty");
}

#[test]
fn many_solids_join_mirror_and_move_to_a_frame() {
    let cube = Solid::cube(1.0, 1.0, 1.0);
    let row: Vec<Solid> = (0..3).map(|i| cube.translate([f64::from(i), 0.0, 0.0])).collect();
    assert_eq!(Solid::union_all(&row).bounds(), [[-0.5, -0.5, -0.5], [2.5, 0.5, 0.5]]);
    assert_eq!(format!("{:?}", Solid::union_all(&[])), "empty");
    assert_eq!(cube.translate([2.0, 0.0, 0.0]).mirror([1.0, 0.0, 0.0]).bounds()[0][0], -2.5);
    let long = Solid::cube(2.0, 1.0, 1.0);
    let placed = long.place(&Frame::turned([0.0, 0.0, 5.0], [0.0, 0.0, 90.0]));
    assert_eq!(placed.bounds(), [[-0.5, -1.0, 4.5], [0.5, 1.0, 5.5]]);
    let hull = cube.union(&cube.translate([3.0, 0.0, 0.0])).hull();
    assert_eq!(hull.bounds(), [[-0.5, -0.5, -0.5], [3.5, 0.5, 0.5]]);
    assert!(hull.triangles() < 24, "the hull is one box, not two");
    assert_eq!(Solid::cube(-1.0, 1.0, 1.0).checked().unwrap_err(), "the kernel gave an error: the construction is not valid");
}

#[test]
fn seeds_and_sockets_have_a_text_form() {
    assert_eq!(Seed::from_text(" 42 "), Ok(Seed(42)));
    assert_eq!(Seed(42).to_text(), "42");
    assert_eq!(Seed::default_value(), Seed(0));
    assert_eq!(Seed::from_text("-1"), Err("\"-1\" is not a seed: write a whole number from 0".into()));
    let sockets = Sockets::from_text("muzzle 0 0 2; grip 1 2 3 0 0 90\n\n stock -1 0 0.5").unwrap();
    assert_eq!(format!("{sockets:?}"), "3 sockets: muzzle, grip, stock");
    assert_eq!(sockets.to_text(), "muzzle 0 0 2; grip 1 2 3 0 0 90; stock -1 0 0.5");
    assert_eq!(Sockets::from_text(&sockets.to_text()), Ok(sockets.clone()));
    assert_eq!(Sockets::from_text("a 1 2"), Err("write \"a 1 2\" as a name and 3 or 6 numbers".into()));
    assert_eq!(Sockets::from_text("a 1 x 2"), Err("\"x\" in \"a 1 x 2\" is not a number".into()));
    assert_eq!(Sockets::from_text(" ; "), Ok(Sockets::default()));
    for angles in [[30.0, 20.0, 40.0], [30.0, 90.0, 40.0], [10.0, -90.0, 20.0], [-150.0, 45.0, 170.0]] {
        let frame = Frame::turned([1.0, 2.0, 3.0], angles);
        let again = Frame::turned([1.0, 2.0, 3.0], frame.angles());
        for (a, b) in [(frame.x, again.x), (frame.y, again.y), (frame.z, again.z)] {
            assert!(a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-9), "{angles:?}");
        }
    }
}

#[test]
fn the_export_writes_glb_obj_and_stl() {
    use export::{Format, bytes};
    let (a, b) = (Solid::cube(2.0, 2.0, 2.0).as_part(), Solid::cube(1.0, 1.0, 1.0).translate([1.5, 0.0, 0.0]).as_part());
    let mesh = a.union(&b).mesh();
    let sockets = Sockets::from_text("muzzle 0 0 2; grip\"x 1 2 3").unwrap();
    let glb = bytes(&mesh, &sockets, &[], Format::Glb).unwrap();
    let word = |at: usize| u32::from_le_bytes(glb[at..at + 4].try_into().unwrap());
    assert_eq!((&glb[..4], word(4), word(8) as usize), (&b"glTF"[..], 2, glb.len()));
    let json = std::str::from_utf8(&glb[20..20 + word(12) as usize]).unwrap();
    // Z up goes to Y up: the muzzle at z = 2 is at y = 2.
    assert!(json.contains(r#""sockets":{"muzzle":{"origin":[0,2,0]"#), "{json}");
    assert!(json.contains(r#""grip\"x":{"origin":[1,3,-2]"#), "{json}");
    assert_eq!(json.matches("POSITION").count(), 2, "one primitive for each part");
    assert_eq!(word(20 + word(12) as usize) as usize, mesh.triangles() * 3 * 32);
    assert!(!json.contains("material"), "no colors give no materials");
    // The two parts share a red material: sRGB 0xff0000 is linear [1, 0, 0].
    let glb = bytes(&mesh, &sockets, &[0xff_0000, 0xff_0000], Format::Glb).unwrap();
    let word = |at: usize| u32::from_le_bytes(glb[at..at + 4].try_into().unwrap());
    let json = std::str::from_utf8(&glb[20..20 + word(12) as usize]).unwrap();
    assert!(json.contains(r#""materials":[{"pbrMetallicRoughness":{"baseColorFactor":[1,0,0,1],"metallicFactor":0}}]"#), "{json}");
    assert_eq!(json.matches(r#""material":0"#).count(), 2, "{json}");
    let obj = String::from_utf8(bytes(&mesh, &sockets, &[], Format::Obj).unwrap()).unwrap();
    assert_eq!(obj.lines().filter(|l| l.starts_with("v ")).count(), mesh.triangles() * 3);
    assert_eq!(obj.lines().filter(|l| l.starts_with("f ")).count(), mesh.triangles());
    assert_eq!(obj.lines().filter(|l| l.starts_with("g part")).count(), 2);
    let stl = bytes(&mesh, &sockets, &[], Format::Stl).unwrap();
    assert_eq!(stl.len(), 84 + 50 * mesh.triangles());
    assert!(bytes(&Solid::empty().mesh(), &sockets, &[], Format::Stl).is_err());
    assert!(Format::of(std::path::Path::new("gun.fbx")).is_err());
    assert_eq!(Format::of(std::path::Path::new("Gun.GLB")), Ok(Format::Glb));
}

#[test]
fn the_export_replaces_a_file_and_keeps_no_temporary_file() {
    let folder = std::env::temp_dir().join(format!("patina-export-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("box.stl");
    let sockets = Sockets::default();
    export::write(&path, &Solid::cube(1.0, 1.0, 1.0).mesh(), &sockets, &[]).unwrap();
    export::write(&path, &Solid::cube(1.0, 1.0, 1.0).union(&Solid::cube(1.0, 1.0, 1.0).translate([2.0, 0.0, 0.0])).mesh(), &sockets, &[]).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 84 + 50 * 24);
    assert!(export::write(&folder.join("box.txt"), &Solid::cube(1.0, 1.0, 1.0).mesh(), &sockets, &[]).is_err());
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);
    std::fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn a_scene_places_meshes_and_bakes_each_group() {
    let cube = Solid::cube(2.0, 2.0, 2.0).mesh();
    let wide = Frame { x: [2.0, 0.0, 0.0], ..Frame::at([10.0, 0.0, 1.0]) };
    let item = |name: &str, places: Vec<Place>| Item { name: name.into(), mesh: cube.clone(), color: 0x80_8080, places };
    let a = Scene::new([item("a", vec![Place::new(&Frame::at([0.0; 3]), 1), Place::new(&wide, 2)]), item("none", vec![])]);
    let b = Scene::new([item("b", vec![Place::new(&Frame::at([0.0, 5.0, 0.0]), 0)])]);
    // An item with no place is not part of the scene.
    assert_eq!(a.items().len(), 1);
    let both = a.merge(&b);
    assert_eq!(format!("{both:?}"), "2 items, 3 places, 36 triangles, bounds -1 -1 -1 to 12 6 2");
    assert_eq!((both.groups(), both.places()), (vec![1, 2], 3));
    assert_ne!(both.serial(), a.serial());
    // A group bakes only its places, with the scale of the frame and normals of length 1.
    let (two, colors) = both.bake(Some(2));
    assert_eq!((two.triangles(), Scene::new([]).bake(None).0.triangles(), colors), (12, 0, vec![0x80_8080]));
    assert!(two.positions.iter().all(|p| (8.0..=12.0).contains(&p[0])));
    assert!(two.normals.iter().all(|n| (n[0] * n[0] + n[1] * n[1] + n[2] * n[2] - 1.0).abs() < 1e-6));
    let (all, colors) = both.bake(None);
    assert_eq!(colors.len(), 2);
    assert_eq!((all.triangles(), all.parts.iter().max()), (36, Some(&1)));
    // The hash sees a change of a place.
    let moved = Scene::new([item("b", vec![Place::new(&Frame::at([0.0, 5.5, 0.0]), 0)])]);
    assert_eq!(b.hash(), b.clone().hash());
    assert_ne!(b.hash(), moved.hash());
}

#[test]
fn a_loft_joins_two_outlines() {
    let volume = |solid: &Solid| -> f64 {
        let mesh = solid.mesh();
        let corner = |i: usize| mesh.positions[i].map(f64::from);
        (0..mesh.triangles()).map(|t| {
            let [a, b, c] = [corner(3 * t), corner(3 * t + 1), corner(3 * t + 2)];
            a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])
        }).sum::<f64>() / 6.0
    };
    let near = |a: f64, b: f64| (a - b).abs() < 1e-4;
    let rect = Sketch::rectangle(2.0, 1.0);
    let block = Solid::loft(&rect, &rect, 3.0).unwrap();
    assert_eq!(block.bounds(), [[-1.0, -0.5, 0.0], [1.0, 0.5, 3.0]]);
    assert!(near(volume(&block), 6.0), "{}", volume(&block));
    // The corners of the two squares meet, so the loft is a frustum: 3 / 3 x (4 + 1 + 2).
    let frustum = Solid::loft(&Sketch::rectangle(2.0, 2.0), &Sketch::rectangle(1.0, 1.0), 3.0).unwrap();
    assert!(near(volume(&frustum), 7.0), "{}", volume(&frustum));
    let round = Solid::loft(&Sketch::rectangle(2.0, 2.0), &Sketch::circle(1.0, 16), 2.0).unwrap();
    let [low, high] = round.bounds();
    assert!(near(low[0], -1.0) && near(high[1], 1.0) && high[2] == 2.0, "{round:?}");
    assert!(volume(&round) > 2.0 * 3.0 && volume(&round) < 2.0 * 4.0, "{}", volume(&round));
    // A concave outline gets new corners on its edges from the other outline.
    let l = |s: f64| Sketch::polygon(&[[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [1.0, 1.0], [1.0, 2.0], [0.0, 2.0]].map(|[x, y]| [x * s, y * s]));
    let bent = Solid::loft(&l(1.0), &Sketch::circle(0.5, 7).translate([0.6, 0.6]), 1.0).unwrap();
    assert!(volume(&bent) > 0.0 && bent.project().area() > 2.9, "{bent:?}");
    // A hole joins the hole of the other sketch with the nearest center. A hole that moves gives
    // the same area at each height, so the volume does not change.
    let holes = |x: f64| {
        let hole = |at: f64| Solid::cube(1.0, 1.0, 3.0).translate([at, 0.0, 0.0]);
        Solid::cube(4.0, 2.0, 2.0).difference(&hole(-1.0)).difference(&hole(x)).project()
    };
    let moved = Solid::loft(&holes(1.0), &holes(0.8), 2.0).unwrap();
    assert!(near(volume(&moved), 12.0), "{}", volume(&moved));
    let frame = Solid::cube(2.0, 2.0, 2.0).difference(&Solid::cube(1.0, 1.0, 3.0)).project();
    let ring = Solid::cylinder(2.0, 1.2, 1.2, 32).difference(&Solid::cylinder(3.0, 0.4, 0.4, 16)).project();
    let tube = Solid::loft(&frame, &ring, 1.0).unwrap();
    assert!(volume(&tube) > 3.0 && volume(&tube) < 4.1, "{}", volume(&tube));
    let solid = Solid::loft(&frame, &Sketch::circle(1.2, 32), 1.0);
    assert_eq!(solid.unwrap_err(), "the two sketches must have the same number of holes, not 1 and 0");
    let pair = Solid::cube(1.0, 1.0, 1.0).union(&Solid::cube(1.0, 1.0, 1.0).translate([3.0, 0.0, 0.0])).project();
    assert_eq!(Solid::loft(&pair, &rect, 1.0).unwrap_err(), "the bottom sketch must have one outer outline");
    let far = Sketch::polygon(&[[0.0, 0.0], [1e300, 0.0], [0.0, 1e300]]);
    assert_eq!(Solid::loft(&rect, &far, 1.0).unwrap_err(), "the top sketch has a kernel error: the construction is not valid");
    assert_eq!(Solid::loft(&rect, &Sketch::default(), 1.0).unwrap_err(), "the top sketch must have one outer outline");
    assert!(Solid::loft(&rect, &rect, 0.0).is_err() && Solid::loft(&rect, &rect, f64::NAN).is_err());
}
