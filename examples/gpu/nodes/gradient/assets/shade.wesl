// The "invert" feature comes from the parameter of the node (spec 10.3).
@if(!invert) fn shade(t: f32) -> vec3f { return vec3f(t * t); }
@if(invert) fn shade(t: f32) -> vec3f { return vec3f(1.0 - t * t); }
