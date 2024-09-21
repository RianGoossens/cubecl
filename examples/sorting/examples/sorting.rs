fn main() {
    #[cfg(feature = "cuda")]
    sorting::launch::<cubecl::cuda::CudaRuntime>(&Default::default());
    #[cfg(feature = "wgpu")]
    sorting::launch::<cubecl::wgpu::WgpuRuntime>(&Default::default());
}
