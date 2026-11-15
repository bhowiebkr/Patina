use cudarc::driver::PushKernelArg;

use crate::cuda;

const ADD: &str = r#"extern "C" __global__ void add(const float* a, float* b, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) b[i] += a[i];
}"#;

#[test]
fn a_kernel_compiles_and_runs_or_the_error_tells_why() {
    let cuda = match cuda() {
        Ok(cuda) => cuda,
        Err(e) => return assert!(e.contains("CUDA"), "{e}"),
    };
    let add = match cuda.kernel(ADD, "add") {
        Ok(add) => add,
        Err(e) => return assert!(e.contains("NVRTC"), "{e}"),
    };
    let a = cuda.stream.clone_htod(&[1.0f32, 2.0, 3.0]).unwrap();
    let mut b = cuda.stream.clone_htod(&[10.0f32, 20.0, 30.0]).unwrap();
    let n = 3i32;
    let mut launch = cuda.stream.launch_builder(&add);
    launch.arg(&a).arg(&mut b).arg(&n);
    // SAFETY: the kernel reads n values of a and b, and both have n values.
    unsafe { launch.launch(cudarc::driver::LaunchConfig::for_num_elems(3)) }.unwrap();
    assert_eq!(cuda.stream.clone_dtoh(&b).unwrap(), [11.0, 22.0, 33.0]);

    let error = cuda.kernel("extern \"C\" __global__ void bad( {}", "bad").err().unwrap();
    assert!(error.starts_with("bad: ") && error.contains("error"), "{error}");
}
