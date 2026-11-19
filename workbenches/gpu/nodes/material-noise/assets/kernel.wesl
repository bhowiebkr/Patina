//! name: Noise
//! category: Core
//! description: Breakup across the model's surface. Fractal noise is evaluated at each point's 3D
//! position, so it has no UV seams or stretching. Scale counts features along the model's largest
//! side. Stretch makes them longer along one axis, and Warp bends them.
//! output: output Gray "Noise"
//! param: style enum(Smooth|Ridged) = smooth "Style"
//! param: scale float(0.25,512) = 8 "Scale" soft
//! param: octaves int(1,8) = 4 "Octaves"
//! param: gain float(0,1) = 0.5 "Gain"
//! param: stretch float(1,64) = 1 "Stretch" soft
//! param: axis enum(X|Y|Z) = y "Stretch axis"
//! param: warp float(0,8) = 0 "Warp" soft
//! param: seed int(0,1000) = 0 "Seed"
//! domain: mesh

// Octaves are independent, so their sum spreads by the root of the summed squared amplitudes. Dividing by
// that keeps the output's spread the same at any octaves or gain, and a mask's thresholds keep their meaning.
const CONTRAST: f32 = 1.2;
// Not a whole number, and shifted, so octaves never line up on one lattice.
const LACUNARITY: f32 = 2.03;
const SHIFT: vec3<f32> = vec3<f32>(17.1, 31.7, 5.3);
// The mean of abs(n) * 2. Subtracting it keeps Ridged centred like Smooth.
const FOLD_MIDDLE: f32 = 0.3;

fn octave(q: vec3<f32>) -> f32 {
    let n = gradient_noise(q);
    return select(n, FOLD_MIDDLE - abs(n) * 2.0, p_style() == STYLE_RIDGED);
}

fn eval(p: vec2<i32>) -> Out {
    var q = mesh_unit_position(p) * p_scale() + vec3<f32>(hash3(vec3<i32>(p_seed(), 7, 13))) * 97.0;
    q[p_axis()] /= p_stretch();
    if p_warp() > 0.0 {
        q += vec3<f32>(gradient_noise(q + SHIFT), gradient_noise(q - SHIFT), gradient_noise(q + SHIFT.zxy)) * p_warp();
    }
    var sum = 0.0;
    var total = 0.0;
    var amplitude = 1.0;
    for (var i = 0; i < p_octaves(); i++) {
        sum += octave(q) * amplitude;
        total += amplitude * amplitude;
        amplitude *= p_gain();
        q = q * LACUNARITY + SHIFT;
    }
    return Out(clamp(sum / sqrt(total) * CONTRAST + 0.5, 0.0, 1.0));
}
