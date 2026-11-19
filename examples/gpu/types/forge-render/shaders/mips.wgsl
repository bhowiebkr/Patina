// Mip chain of the material planes. `first` copies the three node-output layers into mip 0;
// `down` averages 2x2 texels of the level above into the next one.

@group(0) @binding(0) var src0: texture_2d<f32>;
@group(0) @binding(1) var src1: texture_2d<f32>;
@group(0) @binding(2) var src2: texture_2d<f32>;
@group(0) @binding(3) var top: texture_storage_2d_array<rgba16float, write>;

@group(0) @binding(4) var above: texture_2d_array<f32>;
@group(0) @binding(5) var below: texture_storage_2d_array<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn first(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= textureDimensions(top)) {
        return;
    }
    let p = vec2<i32>(id.xy);
    textureStore(top, p, 0, textureLoad(src0, p, 0));
    textureStore(top, p, 1, textureLoad(src1, p, 0));
    textureStore(top, p, 2, textureLoad(src2, p, 0));
}

@compute @workgroup_size(8, 8, 1)
fn down(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= textureDimensions(below)) {
        return;
    }
    let last = vec2<i32>(textureDimensions(above)) - 1;
    let p = vec2<i32>(id.xy) * 2;
    let layer = i32(id.z);
    var sum = vec4<f32>(0.0);
    for (var i = 0; i < 4; i++) {
        let q = min(p + vec2<i32>(i & 1, i >> 1), last);
        sum += textureLoad(above, q, layer, 0);
    }
    textureStore(below, vec2<i32>(id.xy), layer, sum * 0.25);
}
