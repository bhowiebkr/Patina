//! name: Mask
//! category: Core
//! description: Shapes a driver into a mask or a value range. Breakup shifts the driver, so the edge
//! follows the noise. The output goes from Low to High as the result passes from From to To, with
//! From above To turning the mask over.
//! input: driver Gray "Driver" primary
//! input: breakup Gray "Breakup" default=0.5
//! output: output Gray "Mask"
//! param: amount float(0,4) = 0 "Breakup amount" soft
//! param: from float(0,1) = 0.25 "From"
//! param: to float(0,1) = 0.75 "To"
//! param: low float(0,1) = 0 "Low"
//! param: high float(0,1) = 1 "High"

fn eval(p: vec2<i32>) -> Out {
    let x = gray_driver(p) + (gray_breakup(p) - 0.5) * p_amount();
    let span = p_to() - p_from();
    let t = clamp((x - p_from()) / select(span, 1e-5, abs(span) < 1e-5), 0.0, 1.0);
    return Out(mix(p_low(), p_high(), t * t * (3.0 - 2.0 * t)));
}
