//! name: Combine
//! category: Core
//! description: Joins two masks. Max and Screen give either of them, Min and Multiply give both, and
//! Subtract cuts B out of A.
//! input: a Gray "A" primary
//! input: b Gray "B"
//! output: output Gray "Mask"
//! param: mode enum(Max|Min|Multiply|Screen|Add|Subtract) = max "Mode"
//! param: amount float(0,1) = 1 "B amount"

fn eval(p: vec2<i32>) -> Out {
    let a = gray_a(p);
    let b = gray_b(p) * p_amount();
    var value: f32;
    switch p_mode() {
        case MODE_MIN: { value = min(a, b + 1.0 - p_amount()); }
        case MODE_MULTIPLY: { value = a * (b + 1.0 - p_amount()); }
        case MODE_SCREEN: { value = 1.0 - (1.0 - a) * (1.0 - b); }
        case MODE_ADD: { value = a + b; }
        case MODE_SUBTRACT: { value = a - b; }
        default: { value = max(a, b); }
    }
    return Out(clamp(value, 0.0, 1.0));
}
