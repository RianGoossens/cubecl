use cubecl_core as cubecl;
use cubecl_core::prelude::*;

use super::base::SharedMemories;
use super::config::CmmaConfig;

#[cube]
#[allow(unused_mut)]
pub(crate) fn compute_loop<F: Float, FC: Float>(
    mut shared_memories: SharedMemories<F, FC>,
    config: Comptime<CmmaConfig>,
) {
    let block_size_m = Comptime::map(config, |c| c.block_size_m); // 64
    let block_size_k = Comptime::map(config, |c| c.block_size_k); // 32
    let block_size_n = Comptime::map(config, |c| c.block_size_n); // 64
    let tile_size = Comptime::map(config, |c| c.tile_size); // 16
    let num_tiles_in_k = Comptime::runtime(block_size_k / tile_size); // 32/16 = 2

    // let num_tile_elems = Comptime::runtime((tile_size * tile_size) / sm_vec); // 256 / 4
    let num_tile_elems = UInt::new(256); //Comptime::runtime(tile_size * tile_size); // 16*16 = 256

    let num_tiles_per_row = block_size_m / tile_size; // 64/16 = 4
    let num_tiles_per_col = block_size_n / tile_size; // 64/16 = 4
    let num_tiles = num_tiles_per_row * num_tiles_per_col; // 4*4 = 16

    let n_iterations = Comptime::runtime(num_tiles) / CUBE_DIM_X; // 16/8 = 2
    let num_subcube_per_row =
        Comptime::runtime(block_size_n) / (n_iterations * Comptime::runtime(tile_size)); // 64 / (2*16) = 2

    let subcube_id = UNIT_POS_X; // 0..7
    let tile_row = subcube_id / num_subcube_per_row; // 0..3
    let tile_col_base = (subcube_id % num_subcube_per_row) * n_iterations; //0 or 2

    for n_iter in range(0u32, n_iterations, Comptime::new(false)) {
        // 0..1
        let tile_col = tile_col_base + n_iter; // 0..3

        let accumulate_tile = tile_row * Comptime::runtime(num_tiles_per_row) + tile_col; // 0..3 * 4 + 0..3 -> 0..15
        let accumulate_pos = accumulate_tile * num_tile_elems; // 0..3840
        let accumulate_slice = shared_memories
            .accumulate
            .slice_mut(accumulate_pos, accumulate_pos + num_tile_elems);

        for k_iter in range(0u32, num_tiles_in_k, Comptime::new(false)) {
            // 0..1
            let shared_lhs_tile = tile_row * num_tiles_in_k + k_iter; // 0..7
            let shared_rhs_tile = tile_col * num_tiles_in_k + k_iter; // 0..7
            let shared_lhs_pos = shared_lhs_tile * num_tile_elems; // 0..1792
            let shared_rhs_pos = shared_rhs_tile * num_tile_elems; // 0..1792

            let lhs_slice = shared_memories
                .lhs
                .slice(shared_lhs_pos, shared_lhs_pos + num_tile_elems);
            let rhs_slice = shared_memories
                .rhs
                .slice(shared_rhs_pos, shared_rhs_pos + num_tile_elems);

            // cmma_computation(lhs_slice, rhs_slice, accumulate_slice);
            cmma_row_major_mimic(lhs_slice, rhs_slice, accumulate_slice);
        }
    }
}

#[cube]
pub fn cmma_row_major_mimic<F: Float, FC: Float>(
    lhs: &Slice<FC>,
    rhs: &Slice<FC>,
    out: &mut SliceMut<F>,
) {
    if UNIT_POS_Y < UInt::new(16) {
        let warp_tile = UInt::new(16);
        let unit_tile = UInt::new(4);

        for i in range(0u32, unit_tile, Comptime::new(false)) {
            let row = (UNIT_POS_Y / unit_tile) * unit_tile + i;
            for j in range(0u32, unit_tile, Comptime::new(false)) {
                let col = (UNIT_POS_Y % unit_tile) * unit_tile + j;
                let mut r = FC::new(0.);
                for dot in range(0u32, warp_tile, Comptime::new(false)) {
                    let a = lhs[row * warp_tile + dot];
                    let b = rhs[col + dot * warp_tile];
                    r += a * b;
                }
                out[row * warp_tile + col] += F::cast_from(r);
            }
        }
    }
}

#[cube]
pub fn cmma_computation<F: Float, FC: Float>(
    lhs: &Slice<FC>,
    rhs: &Slice<FC>,
    out: &mut SliceMut<F>,
) {
    let a = cmma::Matrix::<FC>::new(
        cmma::MatrixIdent::A,
        16,
        16,
        16,
        cmma::MatrixLayout::RowMajor,
    );
    let b = cmma::Matrix::<FC>::new(
        cmma::MatrixIdent::B,
        16,
        16,
        16,
        cmma::MatrixLayout::RowMajor,
    );
    let c = cmma::Matrix::<F>::new(
        cmma::MatrixIdent::Accumulator,
        16,
        16,
        16,
        cmma::MatrixLayout::Undefined,
    );
    cmma::fill::<F>(&c, F::new(0.0));
    cmma::load::<FC>(&a, lhs.as_slice(), UInt::new(16));
    cmma::load::<FC>(&b, rhs.as_slice(), UInt::new(16));

    cmma::execute::<FC, FC, F, F>(&a, &b, &c, &c);

    cmma::store::<F>(
        out.as_slice_mut(),
        &c,
        UInt::new(16),
        cmma::MatrixLayout::RowMajor,
    );
}

#[cfg(feature = "export_tests")]
/// Compute loop exported tests
pub mod tests {
    use crate::matmul::{
        cmma::base::SharedMemoriesExpand,
        test_utils::{
            assert_equals, assert_equals_range, create_empty, range_tensor, range_tensor_f16,
        },
    };

    use super::*;

    #[cube(launch)]
    fn compute_loop_mimic_test<F: Float>(lhs: &Tensor<F>, rhs: &Tensor<F>, result: &mut Array<F>) {
        cmma_row_major_mimic(lhs.as_slice(), rhs.as_slice(), result.as_slice_mut());
    }

    #[cube(launch)]
    fn compute_loop_mimic_offseted_test<F: Float>(
        lhs: &Tensor<F>,
        rhs: &Tensor<F>,
        result: &mut Array<F>,
    ) {
        cmma_row_major_mimic(
            lhs.slice(256, 512),
            rhs.slice(256, 512),
            result.slice_mut(768, 1024),
        );
    }

    #[cube(launch)]
    fn compute_loop_cmma_test<F: Float, FC: Float>(
        lhs: &Tensor<FC>,
        rhs: &Tensor<FC>,
        result: &mut Array<F>,
    ) {
        cmma_computation(lhs.as_slice(), rhs.as_slice(), result.as_slice_mut());
    }

    #[cube(launch)]
    fn compute_loop_cmma_offseted_slice_test<F: Float, FC: Float>(
        lhs: &Tensor<FC>,
        rhs: &Tensor<FC>,
        result: &mut Array<F>,
    ) {
        cmma_computation(
            lhs.slice(256, 512),
            rhs.slice(256, 512),
            result.slice_mut(768, 1024),
        );
    }

    #[cube(launch)]
    fn compute_loop_test<F: Float, FC: Float>(
        lhs_tensor: &Tensor<FC>,
        rhs_tensor: &Tensor<FC>,
        accumulate_array: &mut Array<F>,
        m: Comptime<UInt>,
        k: Comptime<UInt>,
        n: Comptime<UInt>,
        config: Comptime<CmmaConfig>,
    ) {
        let mut lhs = SharedMemory::<FC>::new(Comptime::get(m * k));
        let mut rhs = SharedMemory::<FC>::new(Comptime::get(k * n));
        let mut accumulate = SharedMemory::<F>::new(Comptime::get(m * n));
        for i in range(0u32, Comptime::get(m * k), Comptime::new(false)) {
            lhs[i] = lhs_tensor[i];
        }
        for i in range(0u32, Comptime::get(k * n), Comptime::new(false)) {
            rhs[i] = rhs_tensor[i];
        }
        for i in range(0u32, Comptime::get(m * n), Comptime::new(false)) {
            accumulate[i] = F::new(0.);
        }

        let shared_memories = SharedMemories {
            lhs,
            rhs,
            accumulate,
        };

        compute_loop(shared_memories, config);

        for i in range(0u32, Comptime::get(m * n), Comptime::new(false)) {
            accumulate_array[i] = accumulate[i];
        }
    }

    /// Exported test
    pub fn compute_loop_mimic_warp_test<R: Runtime>(device: &R::Device) {
        let lhs = range_tensor::<R>(16, 16, device);
        let rhs = range_tensor::<R>(16, 16, device);
        let results = create_empty::<R>(16, 16, device);
        let cube_dim = CubeDim::new(1, 32, 1);
        let cube_count = CubeCount::Static(1, 1, 1);

        compute_loop_mimic_test::launch::<F32, R>(
            R::client(device),
            cube_count,
            cube_dim,
            TensorArg::new(&lhs.handle, &lhs.strides, &lhs.shape),
            TensorArg::new(&rhs.handle, &rhs.strides, &rhs.shape),
            ArrayArg::new(&results, 256),
        );

        let expected = &[
            19840., 19960., 20080., 20200., 20320., 20440., 20560., 20680., 20800., 20920., 21040.,
            21160., 21280., 21400., 21520., 21640., 50560., 50936., 51312., 51688., 52064., 52440.,
            52816., 53192., 53568., 53944., 54320., 54696., 55072., 55448., 55824., 56200., 81280.,
            81912., 82544., 83176., 83808., 84440., 85072., 85704., 86336., 86968., 87600., 88232.,
            88864., 89496., 90128., 90760., 112000., 112888., 113776., 114664., 115552., 116440.,
            117328., 118216., 119104., 119992., 120880., 121768., 122656., 123544., 124432.,
            125320., 142720., 143864., 145008., 146152., 147296., 148440., 149584., 150728.,
            151872., 153016., 154160., 155304., 156448., 157592., 158736., 159880., 173440.,
            174840., 176240., 177640., 179040., 180440., 181840., 183240., 184640., 186040.,
            187440., 188840., 190240., 191640., 193040., 194440., 204160., 205816., 207472.,
            209128., 210784., 212440., 214096., 215752., 217408., 219064., 220720., 222376.,
            224032., 225688., 227344., 229000., 234880., 236792., 238704., 240616., 242528.,
            244440., 246352., 248264., 250176., 252088., 254000., 255912., 257824., 259736.,
            261648., 263560., 265600., 267768., 269936., 272104., 274272., 276440., 278608.,
            280776., 282944., 285112., 287280., 289448., 291616., 293784., 295952., 298120.,
            296320., 298744., 301168., 303592., 306016., 308440., 310864., 313288., 315712.,
            318136., 320560., 322984., 325408., 327832., 330256., 332680., 327040., 329720.,
            332400., 335080., 337760., 340440., 343120., 345800., 348480., 351160., 353840.,
            356520., 359200., 361880., 364560., 367240., 357760., 360696., 363632., 366568.,
            369504., 372440., 375376., 378312., 381248., 384184., 387120., 390056., 392992.,
            395928., 398864., 401800., 388480., 391672., 394864., 398056., 401248., 404440.,
            407632., 410824., 414016., 417208., 420400., 423592., 426784., 429976., 433168.,
            436360., 419200., 422648., 426096., 429544., 432992., 436440., 439888., 443336.,
            446784., 450232., 453680., 457128., 460576., 464024., 467472., 470920., 449920.,
            453624., 457328., 461032., 464736., 468440., 472144., 475848., 479552., 483256.,
            486960., 490664., 494368., 498072., 501776., 505480., 480640., 484600., 488560.,
            492520., 496480., 500440., 504400., 508360., 512320., 516280., 520240., 524200.,
            528160., 532120., 536080., 540040.,
        ];
        assert_equals::<R>(results, expected, device);
    }

    /// Exported test
    pub fn compute_loop_mimic_offseted_warp_test<R: Runtime>(device: &R::Device) {
        let lhs = range_tensor::<R>(32, 16, device);
        let rhs = range_tensor::<R>(16, 32, device);
        let results = create_empty::<R>(32, 32, device);
        let cube_dim = CubeDim::new(1, 32, 1);
        let cube_count = CubeCount::Static(1, 1, 1);

        compute_loop_mimic_offseted_test::launch::<F32, R>(
            R::client(device),
            cube_count,
            cube_dim,
            TensorArg::new(&lhs.handle, &lhs.strides, &lhs.shape),
            TensorArg::new(&rhs.handle, &rhs.strides, &rhs.shape),
            ArrayArg::new(&results, 1024),
        );

        let expected = &[
            1590656.0, 1594872.0, 1599088.0, 1603304.0, 1607520.0, 1611736.0, 1615952.0, 1620168.0,
            1624384.0, 1628600.0, 1632816.0, 1637032.0, 1641248.0, 1645464.0, 1649680.0, 1653896.0,
            1686912.0, 1691384.0, 1695856.0, 1700328.0, 1704800.0, 1709272.0, 1713744.0, 1718216.0,
            1722688.0, 1727160.0, 1731632.0, 1736104.0, 1740576.0, 1745048.0, 1749520.0, 1753992.0,
            1783168.0, 1787896.0, 1792624.0, 1797352.0, 1802080.0, 1806808.0, 1811536.0, 1816264.0,
            1820992.0, 1825720.0, 1830448.0, 1835176.0, 1839904.0, 1844632.0, 1849360.0, 1854088.0,
            1879424.0, 1884408.0, 1889392.0, 1894376.0, 1899360.0, 1904344.0, 1909328.0, 1914312.0,
            1919296.0, 1924280.0, 1929264.0, 1934248.0, 1939232.0, 1944216.0, 1949200.0, 1954184.0,
            1975680.0, 1980920.0, 1986160.0, 1991400.0, 1996640.0, 2001880.0, 2007120.0, 2012360.0,
            2017600.0, 2022840.0, 2028080.0, 2033320.0, 2038560.0, 2043800.0, 2049040.0, 2054280.0,
            2071936.0, 2077432.0, 2082928.0, 2088424.0, 2093920.0, 2099416.0, 2104912.0, 2110408.0,
            2115904.0, 2121400.0, 2126896.0, 2132392.0, 2137888.0, 2143384.0, 2148880.0, 2154376.0,
            2168192.0, 2173944.0, 2179696.0, 2185448.0, 2191200.0, 2196952.0, 2202704.0, 2208456.0,
            2214208.0, 2219960.0, 2225712.0, 2231464.0, 2237216.0, 2242968.0, 2248720.0, 2254472.0,
            2264448.0, 2270456.0, 2276464.0, 2282472.0, 2288480.0, 2294488.0, 2300496.0, 2306504.0,
            2312512.0, 2318520.0, 2324528.0, 2330536.0, 2336544.0, 2342552.0, 2348560.0, 2354568.0,
            2360704.0, 2366968.0, 2373232.0, 2379496.0, 2385760.0, 2392024.0, 2398288.0, 2404552.0,
            2410816.0, 2417080.0, 2423344.0, 2429608.0, 2435872.0, 2442136.0, 2448400.0, 2454664.0,
            2456960.0, 2463480.0, 2470000.0, 2476520.0, 2483040.0, 2489560.0, 2496080.0, 2502600.0,
            2509120.0, 2515640.0, 2522160.0, 2528680.0, 2535200.0, 2541720.0, 2548240.0, 2554760.0,
            2553216.0, 2559992.0, 2566768.0, 2573544.0, 2580320.0, 2587096.0, 2593872.0, 2600648.0,
            2607424.0, 2614200.0, 2620976.0, 2627752.0, 2634528.0, 2641304.0, 2648080.0, 2654856.0,
            2649472.0, 2656504.0, 2663536.0, 2670568.0, 2677600.0, 2684632.0, 2691664.0, 2698696.0,
            2705728.0, 2712760.0, 2719792.0, 2726824.0, 2733856.0, 2740888.0, 2747920.0, 2754952.0,
            2745728.0, 2753016.0, 2760304.0, 2767592.0, 2774880.0, 2782168.0, 2789456.0, 2796744.0,
            2804032.0, 2811320.0, 2818608.0, 2825896.0, 2833184.0, 2840472.0, 2847760.0, 2855048.0,
            2841984.0, 2849528.0, 2857072.0, 2864616.0, 2872160.0, 2879704.0, 2887248.0, 2894792.0,
            2902336.0, 2909880.0, 2917424.0, 2924968.0, 2932512.0, 2940056.0, 2947600.0, 2955144.0,
            2938240.0, 2946040.0, 2953840.0, 2961640.0, 2969440.0, 2977240.0, 2985040.0, 2992840.0,
            3000640.0, 3008440.0, 3016240.0, 3024040.0, 3031840.0, 3039640.0, 3047440.0, 3055240.0,
            3034496.0, 3042552.0, 3050608.0, 3058664.0, 3066720.0, 3074776.0, 3082832.0, 3090888.0,
            3098944.0, 3107000.0, 3115056.0, 3123112.0, 3131168.0, 3139224.0, 3147280.0, 3155336.0,
        ];
        assert_equals_range::<R>(results, expected, 768..1024, device);
    }

    /// Exported test
    pub fn compute_loop_cmma_warp_test<R: Runtime>(device: &R::Device) {
        let lhs = range_tensor_f16::<R>(16, 16, device);
        let rhs = range_tensor_f16::<R>(16, 16, device);
        let results = create_empty::<R>(16, 16, device);
        let cube_dim = CubeDim::new(1, 32, 1);
        let cube_count = CubeCount::Static(1, 1, 1);

        compute_loop_cmma_test::launch::<F32, F16, R>(
            R::client(device),
            cube_count,
            cube_dim,
            TensorArg::new(&lhs.handle, &lhs.strides, &lhs.shape),
            TensorArg::new(&rhs.handle, &rhs.strides, &rhs.shape),
            ArrayArg::new(&results, 256),
        );

        let expected = &[
            19840., 19960., 20080., 20200., 20320., 20440., 20560., 20680., 20800., 20920., 21040.,
            21160., 21280., 21400., 21520., 21640., 50560., 50936., 51312., 51688., 52064., 52440.,
            52816., 53192., 53568., 53944., 54320., 54696., 55072., 55448., 55824., 56200., 81280.,
            81912., 82544., 83176., 83808., 84440., 85072., 85704., 86336., 86968., 87600., 88232.,
            88864., 89496., 90128., 90760., 112000., 112888., 113776., 114664., 115552., 116440.,
            117328., 118216., 119104., 119992., 120880., 121768., 122656., 123544., 124432.,
            125320., 142720., 143864., 145008., 146152., 147296., 148440., 149584., 150728.,
            151872., 153016., 154160., 155304., 156448., 157592., 158736., 159880., 173440.,
            174840., 176240., 177640., 179040., 180440., 181840., 183240., 184640., 186040.,
            187440., 188840., 190240., 191640., 193040., 194440., 204160., 205816., 207472.,
            209128., 210784., 212440., 214096., 215752., 217408., 219064., 220720., 222376.,
            224032., 225688., 227344., 229000., 234880., 236792., 238704., 240616., 242528.,
            244440., 246352., 248264., 250176., 252088., 254000., 255912., 257824., 259736.,
            261648., 263560., 265600., 267768., 269936., 272104., 274272., 276440., 278608.,
            280776., 282944., 285112., 287280., 289448., 291616., 293784., 295952., 298120.,
            296320., 298744., 301168., 303592., 306016., 308440., 310864., 313288., 315712.,
            318136., 320560., 322984., 325408., 327832., 330256., 332680., 327040., 329720.,
            332400., 335080., 337760., 340440., 343120., 345800., 348480., 351160., 353840.,
            356520., 359200., 361880., 364560., 367240., 357760., 360696., 363632., 366568.,
            369504., 372440., 375376., 378312., 381248., 384184., 387120., 390056., 392992.,
            395928., 398864., 401800., 388480., 391672., 394864., 398056., 401248., 404440.,
            407632., 410824., 414016., 417208., 420400., 423592., 426784., 429976., 433168.,
            436360., 419200., 422648., 426096., 429544., 432992., 436440., 439888., 443336.,
            446784., 450232., 453680., 457128., 460576., 464024., 467472., 470920., 449920.,
            453624., 457328., 461032., 464736., 468440., 472144., 475848., 479552., 483256.,
            486960., 490664., 494368., 498072., 501776., 505480., 480640., 484600., 488560.,
            492520., 496480., 500440., 504400., 508360., 512320., 516280., 520240., 524200.,
            528160., 532120., 536080., 540040.,
        ];
        assert_equals::<R>(results, expected, device);
    }

    /// Exported test
    pub fn compute_loop_cmma_offseted_warp_test<R: Runtime>(device: &R::Device) {
        let lhs = range_tensor_f16::<R>(32, 16, device);
        let rhs = range_tensor_f16::<R>(16, 32, device);
        let results = create_empty::<R>(32, 32, device);
        let cube_dim = CubeDim::new(1, 32, 1);
        let cube_count = CubeCount::Static(1, 1, 1);

        compute_loop_cmma_offseted_slice_test::launch::<F32, F16, R>(
            R::client(device),
            cube_count,
            cube_dim,
            TensorArg::new(&lhs.handle, &lhs.strides, &lhs.shape),
            TensorArg::new(&rhs.handle, &rhs.strides, &rhs.shape),
            ArrayArg::new(&results, 256),
        );

        let expected = &[
            1590656.0, 1594872.0, 1599088.0, 1603304.0, 1607520.0, 1611736.0, 1615952.0, 1620168.0,
            1624384.0, 1628600.0, 1632816.0, 1637032.0, 1641248.0, 1645464.0, 1649680.0, 1653896.0,
            1686912.0, 1691384.0, 1695856.0, 1700328.0, 1704800.0, 1709272.0, 1713744.0, 1718216.0,
            1722688.0, 1727160.0, 1731632.0, 1736104.0, 1740576.0, 1745048.0, 1749520.0, 1753992.0,
            1783168.0, 1787896.0, 1792624.0, 1797352.0, 1802080.0, 1806808.0, 1811536.0, 1816264.0,
            1820992.0, 1825720.0, 1830448.0, 1835176.0, 1839904.0, 1844632.0, 1849360.0, 1854088.0,
            1879424.0, 1884408.0, 1889392.0, 1894376.0, 1899360.0, 1904344.0, 1909328.0, 1914312.0,
            1919296.0, 1924280.0, 1929264.0, 1934248.0, 1939232.0, 1944216.0, 1949200.0, 1954184.0,
            1975680.0, 1980920.0, 1986160.0, 1991400.0, 1996640.0, 2001880.0, 2007120.0, 2012360.0,
            2017600.0, 2022840.0, 2028080.0, 2033320.0, 2038560.0, 2043800.0, 2049040.0, 2054280.0,
            2071936.0, 2077432.0, 2082928.0, 2088424.0, 2093920.0, 2099416.0, 2104912.0, 2110408.0,
            2115904.0, 2121400.0, 2126896.0, 2132392.0, 2137888.0, 2143384.0, 2148880.0, 2154376.0,
            2168192.0, 2173944.0, 2179696.0, 2185448.0, 2191200.0, 2196952.0, 2202704.0, 2208456.0,
            2214208.0, 2219960.0, 2225712.0, 2231464.0, 2237216.0, 2242968.0, 2248720.0, 2254472.0,
            2264448.0, 2270456.0, 2276464.0, 2282472.0, 2288480.0, 2294488.0, 2300496.0, 2306504.0,
            2312512.0, 2318520.0, 2324528.0, 2330536.0, 2336544.0, 2342552.0, 2348560.0, 2354568.0,
            2360704.0, 2366968.0, 2373232.0, 2379496.0, 2385760.0, 2392024.0, 2398288.0, 2404552.0,
            2410816.0, 2417080.0, 2423344.0, 2429608.0, 2435872.0, 2442136.0, 2448400.0, 2454664.0,
            2456960.0, 2463480.0, 2470000.0, 2476520.0, 2483040.0, 2489560.0, 2496080.0, 2502600.0,
            2509120.0, 2515640.0, 2522160.0, 2528680.0, 2535200.0, 2541720.0, 2548240.0, 2554760.0,
            2553216.0, 2559992.0, 2566768.0, 2573544.0, 2580320.0, 2587096.0, 2593872.0, 2600648.0,
            2607424.0, 2614200.0, 2620976.0, 2627752.0, 2634528.0, 2641304.0, 2648080.0, 2654856.0,
            2649472.0, 2656504.0, 2663536.0, 2670568.0, 2677600.0, 2684632.0, 2691664.0, 2698696.0,
            2705728.0, 2712760.0, 2719792.0, 2726824.0, 2733856.0, 2740888.0, 2747920.0, 2754952.0,
            2745728.0, 2753016.0, 2760304.0, 2767592.0, 2774880.0, 2782168.0, 2789456.0, 2796744.0,
            2804032.0, 2811320.0, 2818608.0, 2825896.0, 2833184.0, 2840472.0, 2847760.0, 2855048.0,
            2841984.0, 2849528.0, 2857072.0, 2864616.0, 2872160.0, 2879704.0, 2887248.0, 2894792.0,
            2902336.0, 2909880.0, 2917424.0, 2924968.0, 2932512.0, 2940056.0, 2947600.0, 2955144.0,
            2938240.0, 2946040.0, 2953840.0, 2961640.0, 2969440.0, 2977240.0, 2985040.0, 2992840.0,
            3000640.0, 3008440.0, 3016240.0, 3024040.0, 3031840.0, 3039640.0, 3047440.0, 3055240.0,
            3034496.0, 3042552.0, 3050608.0, 3058664.0, 3066720.0, 3074776.0, 3082832.0, 3090888.0,
            3098944.0, 3107000.0, 3115056.0, 3123112.0, 3131168.0, 3139224.0, 3147280.0, 3155336.0,
        ];
        assert_equals_range::<R>(results, expected, 768..1024, device);
    }

    /// Exported test
    pub fn compute_loop_k_test<R: Runtime>(device: &R::Device) {
        let lhs = range_tensor_f16::<R>(16, 32, device);
        let rhs = range_tensor_f16::<R>(32, 16, device);
        let results = create_empty::<R>(16, 16, device);
        let cube_dim = CubeDim::new(1, 32, 1);
        let cube_count = CubeCount::Static(1, 1, 1);

        let config = CmmaConfig {
            block_size_m: UInt::new(64),
            block_size_k: UInt::new(32),
            block_size_n: UInt::new(64),
            check_m_bounds: false,
            check_k_bounds: false,
            check_n_bounds: false,
            tile_size: UInt::new(16),
            sm_vec: UInt::new(4),
            lhs_transposed: false,
            rhs_transposed: false,
            unroll: false,
        };

        compute_loop_test::launch::<F32, F16, R>(
            R::client(device),
            cube_count,
            cube_dim,
            TensorArg::new(&lhs.handle, &lhs.strides, &lhs.shape),
            TensorArg::new(&rhs.handle, &rhs.strides, &rhs.shape),
            ArrayArg::new(&results, 256),
            UInt::new(64),
            UInt::new(32),
            UInt::new(64),
            config,
        );

        let expected = &[
            1610496., 1614832., 1619168., 1623504., 1627840., 1632176., 1636512., 1640848.,
            1645184., 1649520., 1653856., 1658192., 1662528., 1666864., 1671200., 1675536.,
            1737472., 1742320., 1747168., 1752016., 1756864., 1761712., 1766560., 1771408.,
            1776256., 1781104., 1785952., 1790800., 1795648., 1800496., 1805344., 1810192.,
            1864448., 1869808., 1875168., 1880528., 1885888., 1891248., 1896608., 1901968.,
            1907328., 1912688., 1918048., 1923408., 1928768., 1934128., 1939488., 1944848.,
            1991424., 1997296., 2003168., 2009040., 2014912., 2020784., 2026656., 2032528.,
            2038400., 2044272., 2050144., 2056016., 2061888., 2067760., 2073632., 2079504.,
            2118400., 2124784., 2131168., 2137552., 2143936., 2150320., 2156704., 2163088.,
            2169472., 2175856., 2182240., 2188624., 2195008., 2201392., 2207776., 2214160.,
            2245376., 2252272., 2259168., 2266064., 2272960., 2279856., 2286752., 2293648.,
            2300544., 2307440., 2314336., 2321232., 2328128., 2335024., 2341920., 2348816.,
            2372352., 2379760., 2387168., 2394576., 2401984., 2409392., 2416800., 2424208.,
            2431616., 2439024., 2446432., 2453840., 2461248., 2468656., 2476064., 2483472.,
            2499328., 2507248., 2515168., 2523088., 2531008., 2538928., 2546848., 2554768.,
            2562688., 2570608., 2578528., 2586448., 2594368., 2602288., 2610208., 2618128.,
            2626304., 2634736., 2643168., 2651600., 2660032., 2668464., 2676896., 2685328.,
            2693760., 2702192., 2710624., 2719056., 2727488., 2735920., 2744352., 2752784.,
            2753280., 2762224., 2771168., 2780112., 2789056., 2798000., 2806944., 2815888.,
            2824832., 2833776., 2842720., 2851664., 2860608., 2869552., 2878496., 2887440.,
            2880256., 2889712., 2899168., 2908624., 2918080., 2927536., 2936992., 2946448.,
            2955904., 2965360., 2974816., 2984272., 2993728., 3003184., 3012640., 3022096.,
            3007232., 3017200., 3027168., 3037136., 3047104., 3057072., 3067040., 3077008.,
            3086976., 3096944., 3106912., 3116880., 3126848., 3136816., 3146784., 3156752.,
            3134208., 3144688., 3155168., 3165648., 3176128., 3186608., 3197088., 3207568.,
            3218048., 3228528., 3239008., 3249488., 3259968., 3270448., 3280928., 3291408.,
            3261184., 3272176., 3283168., 3294160., 3305152., 3316144., 3327136., 3338128.,
            3349120., 3360112., 3371104., 3382096., 3393088., 3404080., 3415072., 3426064.,
            3388160., 3399664., 3411168., 3422672., 3434176., 3445680., 3457184., 3468688.,
            3480192., 3491696., 3503200., 3514704., 3526208., 3537712., 3549216., 3560720.,
            3515136., 3527152., 3539168., 3551184., 3563200., 3575216., 3587232., 3599248.,
            3611264., 3623280., 3635296., 3647312., 3659328., 3671344., 3683360., 3695376.,
        ];
        assert_equals::<R>(results, expected, device);
    }

    /// Exported test
    pub fn compute_loop_warp_test<R: Runtime>(device: &R::Device) {
        let lhs = range_tensor_f16::<R>(16, 32, device);
        let rhs = range_tensor_f16::<R>(32, 32, device);
        let results = create_empty::<R>(16, 32, device);
        let cube_dim = CubeDim::new(1, 32, 1);
        let cube_count = CubeCount::Static(1, 1, 1);

        let config = CmmaConfig {
            block_size_m: UInt::new(64),
            block_size_k: UInt::new(32),
            block_size_n: UInt::new(64),
            check_m_bounds: false,
            check_k_bounds: false,
            check_n_bounds: false,
            tile_size: UInt::new(16),
            sm_vec: UInt::new(4),
            lhs_transposed: false,
            rhs_transposed: false,
            unroll: false,
        };

        compute_loop_test::launch::<F32, F16, R>(
            R::client(device),
            cube_count,
            cube_dim,
            TensorArg::new(&lhs.handle, &lhs.strides, &lhs.shape),
            TensorArg::new(&rhs.handle, &rhs.strides, &rhs.shape),
            ArrayArg::new(&results, 512),
            UInt::new(64),
            UInt::new(32),
            UInt::new(64),
            config,
        );

        let expected = &[
            1610496., 1614832., 1619168., 1623504., 1627840., 1632176., 1636512., 1640848.,
            1645184., 1649520., 1653856., 1658192., 1662528., 1666864., 1671200., 1675536.,
            1737472., 1742320., 1747168., 1752016., 1756864., 1761712., 1766560., 1771408.,
            1776256., 1781104., 1785952., 1790800., 1795648., 1800496., 1805344., 1810192.,
            1864448., 1869808., 1875168., 1880528., 1885888., 1891248., 1896608., 1901968.,
            1907328., 1912688., 1918048., 1923408., 1928768., 1934128., 1939488., 1944848.,
            1991424., 1997296., 2003168., 2009040., 2014912., 2020784., 2026656., 2032528.,
            2038400., 2044272., 2050144., 2056016., 2061888., 2067760., 2073632., 2079504.,
            2118400., 2124784., 2131168., 2137552., 2143936., 2150320., 2156704., 2163088.,
            2169472., 2175856., 2182240., 2188624., 2195008., 2201392., 2207776., 2214160.,
            2245376., 2252272., 2259168., 2266064., 2272960., 2279856., 2286752., 2293648.,
            2300544., 2307440., 2314336., 2321232., 2328128., 2335024., 2341920., 2348816.,
            2372352., 2379760., 2387168., 2394576., 2401984., 2409392., 2416800., 2424208.,
            2431616., 2439024., 2446432., 2453840., 2461248., 2468656., 2476064., 2483472.,
            2499328., 2507248., 2515168., 2523088., 2531008., 2538928., 2546848., 2554768.,
            2562688., 2570608., 2578528., 2586448., 2594368., 2602288., 2610208., 2618128.,
            2626304., 2634736., 2643168., 2651600., 2660032., 2668464., 2676896., 2685328.,
            2693760., 2702192., 2710624., 2719056., 2727488., 2735920., 2744352., 2752784.,
            2753280., 2762224., 2771168., 2780112., 2789056., 2798000., 2806944., 2815888.,
            2824832., 2833776., 2842720., 2851664., 2860608., 2869552., 2878496., 2887440.,
            2880256., 2889712., 2899168., 2908624., 2918080., 2927536., 2936992., 2946448.,
            2955904., 2965360., 2974816., 2984272., 2993728., 3003184., 3012640., 3022096.,
            3007232., 3017200., 3027168., 3037136., 3047104., 3057072., 3067040., 3077008.,
            3086976., 3096944., 3106912., 3116880., 3126848., 3136816., 3146784., 3156752.,
            3134208., 3144688., 3155168., 3165648., 3176128., 3186608., 3197088., 3207568.,
            3218048., 3228528., 3239008., 3249488., 3259968., 3270448., 3280928., 3291408.,
            3261184., 3272176., 3283168., 3294160., 3305152., 3316144., 3327136., 3338128.,
            3349120., 3360112., 3371104., 3382096., 3393088., 3404080., 3415072., 3426064.,
            3388160., 3399664., 3411168., 3422672., 3434176., 3445680., 3457184., 3468688.,
            3480192., 3491696., 3503200., 3514704., 3526208., 3537712., 3549216., 3560720.,
            3515136., 3527152., 3539168., 3551184., 3563200., 3575216., 3587232., 3599248.,
            3611264., 3623280., 3635296., 3647312., 3659328., 3671344., 3683360., 3695376.,
            3830528., 3834864., 3839200., 3843536., 3847872., 3852208., 3856544., 3860880.,
            3865216., 3869552., 3873888., 3878224., 3882560., 3886896., 3891232., 3895568.,
            4219648., 4224496., 4229344., 4234192., 4239040., 4243888., 4248736., 4253584.,
            4258432., 4263280., 4268128., 4272976., 4277824., 4282672., 4287520., 4292368.,
            4608768., 4614128., 4619488., 4624848., 4630208., 4635568., 4640928., 4646288.,
            4651648., 4657008., 4662368., 4667728., 4673088., 4678448., 4683808., 4689168.,
            4997888., 5003760., 5009632., 5015504., 5021376., 5027248., 5033120., 5038992.,
            5044864., 5050736., 5056608., 5062480., 5068352., 5074224., 5080096., 5085968.,
            5387008., 5393392., 5399776., 5406160., 5412544., 5418928., 5425312., 5431696.,
            5438080., 5444464., 5450848., 5457232., 5463616., 5470000., 5476384., 5482768.,
            5776128., 5783024., 5789920., 5796816., 5803712., 5810608., 5817504., 5824400.,
            5831296., 5838192., 5845088., 5851984., 5858880., 5865776., 5872672., 5879568.,
            6165248., 6172656., 6180064., 6187472., 6194880., 6202288., 6209696., 6217104.,
            6224512., 6231920., 6239328., 6246736., 6254144., 6261552., 6268960., 6276368.,
            6554368., 6562288., 6570208., 6578128., 6586048., 6593968., 6601888., 6609808.,
            6617728., 6625648., 6633568., 6641488., 6649408., 6657328., 6665248., 6673168.,
            6943488., 6951920., 6960352., 6968784., 6977216., 6985648., 6994080., 7002512.,
            7010944., 7019376., 7027808., 7036240., 7044672., 7053104., 7061536., 7069968.,
            7332608., 7341552., 7350496., 7359440., 7368384., 7377328., 7386272., 7395216.,
            7404160., 7413104., 7422048., 7430992., 7439936., 7448880., 7457824., 7466768.,
            7721728., 7731184., 7740640., 7750096., 7759552., 7769008., 7778464., 7787920.,
            7797376., 7806832., 7816288., 7825744., 7835200., 7844656., 7854112., 7863568.,
            8110848., 8120816., 8130784., 8140752., 8150720., 8160688., 8170656., 8180624.,
            8190592., 8200560., 8210528., 8220496., 8230464., 8240432., 8250400., 8260368.,
            8499968., 8510448., 8520928., 8531408., 8541888., 8552368., 8562848., 8573328.,
            8583808., 8594288., 8604768., 8615248., 8625728., 8636208., 8646688., 8657168.,
            8889088., 8900080., 8911072., 8922064., 8933056., 8944048., 8955040., 8966032.,
            8977024., 8988016., 8999008., 9010000., 9020992., 9031984., 9042976., 9053968.,
            9278208., 9289712., 9301216., 9312720., 9324224., 9335728., 9347232., 9358736.,
            9370240., 9381744., 9393248., 9404752., 9416256., 9427760., 9439264., 9450768.,
            9667328., 9679344., 9691360., 9703376., 9715392., 9727408., 9739424., 9751440.,
            9763456., 9775472., 9787488., 9799504., 9811520., 9823536., 9835552., 9847568.,
        ];
        assert_equals::<R>(results, expected, device);
    }
}