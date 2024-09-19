use cubecl_core as cubecl;
use cubecl_core::prelude::*;

use super::{
    base::{Fragments, RuntimeCmmaInfo, SharedMemories},
    compute_loop::base::compute_loop,
    config::ComptimeCmmaInfo,
    load_shared_memory::load_to_shared_memories,
    write_output::{base::OutputWriter, large_smem::LargeSmemWriter, reuse_smem::ReuseSmemWriter},
};

#[cube]
pub(crate) fn block_loop<F: Float, FC: Float>(
    lhs: &Tensor<F>,
    rhs: &Tensor<F>,
    out: &mut Tensor<F>,
    shared_memories: SharedMemories<FC>,
    mut fragments: Fragments<F, FC>,
    runtime_info: RuntimeCmmaInfo,
    #[comptime] comptime_info: ComptimeCmmaInfo,
) {
    let block_size_k = comptime_info.block_size_k;
    let write_out_reuse_smem = comptime_info.write_out_strategy;

    // Equals ceil(dims.k / block_size_k)
    let dims = runtime_info.dims;
    let num_loops = (dims.k + block_size_k - 1) / block_size_k;

    for block in 0..num_loops {
        let k_offset = block * block_size_k;

        load_to_shared_memories::<F, FC>(
            lhs,
            rhs,
            k_offset,
            shared_memories,
            runtime_info,
            comptime_info,
        );

        sync_units();

        compute_loop::<F, FC>(
            shared_memories,
            &mut fragments,
            runtime_info.ids,
            comptime_info,
        );

        sync_units();
    }

    if write_out_reuse_smem == 0 {
        LargeSmemWriter::write_to_output(out, fragments.accumulators, runtime_info, comptime_info);
    } else {
        ReuseSmemWriter::write_to_output(out, fragments.accumulators, runtime_info, comptime_info);
    }
}
