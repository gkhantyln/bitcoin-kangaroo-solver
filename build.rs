fn main() {
    #[cfg(feature = "gpu")]
    {
        println!("cargo:rerun-if-changed=kernels/kangaroo.cu");
        cc::Build::new()
            .cuda(true)
            .file("kernels/kangaroo.cu")
            .flag("-arch=sm_75")
            .flag("-O3")
            .flag("-ptx")
            .compile("kangaroo_kernel");
        println!("cargo:warning=CUDA kernel compiled successfully");
    }
}
