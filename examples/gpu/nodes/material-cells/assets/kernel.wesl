//! name: Cells
//! category: Core
//! description: Separate features scattered through the model's 3D space, so they have no UV seams.
//! Dots are blobs and read as spots. Sheets are thin discs, and a surface cuts a disc along a line, so
//! they read as scratches. Cracks are the walls between neighbouring cells. Each is 1 at the feature's
//! middle, falling to 0 at its edge. Id gives every cell its own random value.
//! output: output Gray "Cells"
//! output: id Gray "Cell id"
//! param: shape enum(Dots|Sheets|Cracks) = dots "Shape"
//! param: scale float(0.25,512) = 16 "Scale" soft
//! param: size float(0,1) = 0.5 "Size"
//! param: variation float(0,1) = 0.5 "Size variation"
//! param: width float(0,1) = 0.05 "Width"
//! param: density float(0,1) = 1 "Density"
//! param: stretch float(1,64) = 1 "Stretch" soft
//! param: axis enum(X|Y|Z) = y "Stretch axis"
//! param: seed int(0,1000) = 0 "Seed"
//! domain: mesh

fn random(cell: vec3<i32>, salt: i32) -> vec3<f32> {
    return vec3<f32>(hash3(cell + vec3<i32>(p_seed() * 131 + salt, salt * 7, p_seed() - salt)));
}

fn eval(p: vec2<i32>) -> Out {
    var q = mesh_unit_position(p) * p_scale();
    q[p_axis()] /= p_stretch();
    let home = vec3<i32>(floor(q));
    var value = 0.0;
    var nearest = 1e9;
    var second = 1e9;
    var id = 0.0;
    for (var z = -1; z <= 1; z++) {
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                let cell = home + vec3<i32>(x, y, z);
                let d = q - (vec3<f32>(cell) + 0.5 + random(cell, 1) * 0.5);
                let squared = dot(d, d);
                if squared < nearest {
                    second = nearest;
                    nearest = squared;
                    id = random(cell, 2).x * 0.5 + 0.5;
                } else if squared < second {
                    second = squared;
                }
                let dice = random(cell, 3);
                if dice.x * 0.5 + 0.5 > p_density() {
                    continue;
                }
                let radius = p_size() * mix(1.0, dice.y * 0.5 + 0.5, p_variation());
                if p_shape() == SHAPE_DOTS {
                    value = max(value, 1.0 - sqrt(squared) / max(radius, 1e-4));
                } else if p_shape() == SHAPE_SHEETS {
                    let n = normalize(random(cell, 4) + vec3<f32>(1e-3));
                    let across = dot(d, n);
                    let along = sqrt(max(squared - across * across, 0.0));
                    value = max(value, (1.0 - abs(across) / max(p_width(), 1e-4)) * (1.0 - smoothstep(radius * 0.5, radius, along)));
                }
            }
        }
    }
    if p_shape() == SHAPE_CRACKS {
        value = 1.0 - (sqrt(second) - sqrt(nearest)) / max(p_width(), 1e-4);
    }
    return Out(clamp(value, 0.0, 1.0), id);
}
