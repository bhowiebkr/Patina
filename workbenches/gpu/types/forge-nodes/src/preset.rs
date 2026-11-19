use crate::{Exposed, Graph, NodeId, Registry};

fn node(
    g: &mut Graph,
    kind: &str,
    title: &str,
    pos: [f32; 2],
    params: &[(&str, &[f32])],
) -> NodeId {
    let id = g.add(kind, pos);
    if let Some(n) = g.node_mut(id) {
        n.title = title.to_owned();
        for (param, value) in params {
            n.params.insert((*param).to_owned(), value.to_vec());
        }
    }
    id
}

fn link(g: &mut Graph, r: &Registry, from: NodeId, output: &str, to: NodeId, input: &str) {
    let linked = g.connect(r, from, output, to, input);
    debug_assert!(
        linked.is_ok(),
        "preset link {output} -> {input}: {linked:?}"
    );
}

fn expose(g: &mut Graph, node: NodeId, param: &str, label: &str) {
    g.exposed.push(Exposed {
        node,
        param: param.to_owned(),
        label: label.to_owned(),
    });
}

fn material(
    g: &mut Graph,
    title: &str,
    pos: [f32; 2],
    color: [f32; 3],
    rough: f32,
    metal: f32,
    height: f32,
) -> NodeId {
    let [r, gr, b] = color;
    node(
        g,
        "material",
        title,
        pos,
        &[
            ("basecolor_value", &[r, gr, b, 1.0]),
            ("roughness_value", &[rough]),
            ("metallic_value", &[metal]),
            ("height_value", &[height]),
        ],
    )
}

/// One masked Material Blend; the mask is the product of a surface fact and a noise.
fn masked_layer(
    g: &mut Graph,
    r: &Registry,
    [title, fact_title]: [&str; 2],
    [background, foreground]: [NodeId; 2],
    fact: &[f32],
    breakup: NodeId,
    y: f32,
) -> NodeId {
    let x = g.node(background).map_or(0.0, |n| n.pos[0]) + 320.0;
    let surface = node(g, "surface", fact_title, [x - 320.0, y], &[("fact", fact)]);
    let mask = node(g, "combine", "Mask", [x - 160.0, y], &[("mode", &[2.0])]);
    let blend = node(g, "material_blend", title, [x, y - 200.0], &[]);
    link(g, r, surface, "output", mask, "a");
    link(g, r, breakup, "output", mask, "b");
    link(g, r, background, "material", blend, "background");
    link(g, r, foreground, "material", blend, "foreground");
    link(g, r, mask, "output", blend, "mask");
    blend
}

/// Worn orange paint over metal with grime and dust layers, built from the core nodes.
pub fn sample_material() -> Graph {
    let r = Registry::new();
    let mut g = Graph {
        name: "Worn orange paint".to_owned(),
        ..Default::default()
    };
    let paint = material(
        &mut g,
        "Orange paint",
        [0.0, 0.0],
        [0.86, 0.42, 0.08],
        0.45,
        0.0,
        0.5,
    );
    let metal = material(
        &mut g,
        "Bare metal",
        [0.0, 220.0],
        [0.6, 0.6, 0.62],
        0.35,
        1.0,
        0.45,
    );
    let grime_fill = material(
        &mut g,
        "Grime fill",
        [320.0, 220.0],
        [0.1, 0.08, 0.06],
        0.85,
        0.0,
        0.5,
    );
    let dust_fill = material(
        &mut g,
        "Dust fill",
        [640.0, 220.0],
        [0.62, 0.58, 0.5],
        0.95,
        0.0,
        0.5,
    );
    let wear_noise = node(
        &mut g,
        "noise",
        "Wear breakup",
        [-320.0, 500.0],
        &[("scale", &[48.0])],
    );
    let grime_noise = node(
        &mut g,
        "noise",
        "Grime breakup",
        [-320.0, 700.0],
        &[("scale", &[12.0])],
    );
    // `fact` values: 0 Enclosed, 2 Edge, 3 Facing.
    let wear = masked_layer(
        &mut g,
        &r,
        ["Edge wear", "Edges"],
        [paint, metal],
        &[2.0],
        wear_noise,
        500.0,
    );
    let grime = masked_layer(
        &mut g,
        &r,
        ["Grime", "Crevices"],
        [wear, grime_fill],
        &[0.0],
        grime_noise,
        700.0,
    );
    let dust = masked_layer(
        &mut g,
        &r,
        ["Dust", "Up faces"],
        [grime, dust_fill],
        &[3.0],
        grime_noise,
        900.0,
    );
    let out = node(&mut g, "output", "", [1280.0, 0.0], &[]);
    link(&mut g, &r, dust, "material", out, "material");
    expose(&mut g, paint, "basecolor_value", "Paint colour");
    expose(&mut g, paint, "roughness_value", "Paint roughness");
    expose(&mut g, wear, "opacity", "Edge wear");
    expose(&mut g, grime, "opacity", "Grime");
    expose(&mut g, dust, "opacity", "Dust");
    expose(&mut g, grime_noise, "scale", "Grime scale");
    g
}

/// A new material: one grey Material node into the Output.
pub fn blank_material(r: &Registry, name: &str) -> Graph {
    let mut g = Graph {
        name: name.to_owned(),
        ..Default::default()
    };
    let base = material(&mut g, "Base", [0.0, 0.0], [0.5, 0.5, 0.5], 0.5, 0.0, 0.5);
    let out = node(&mut g, "output", "", [320.0, 0.0], &[]);
    link(&mut g, r, base, "material", out, "material");
    g
}
