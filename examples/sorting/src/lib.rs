use std::time::Instant;

use cubecl::prelude::*;

use rand::{thread_rng, Rng};

#[cube]
pub trait Sort: Send + Sync + 'static {
    fn sort<N: Numeric>(input: &mut Array<N>);
}

pub struct OddEvenTransitionSort;

#[cube]
impl Sort for OddEvenTransitionSort {
    fn sort<N: Numeric>(input: &mut Array<N>) {
        let len = input.len();
        let num_chunks = f32::ceil(len as f32 / CUBE_DIM as f32 / 2.) as u32;
        if ABSOLUTE_POS < len - 1 {
            for i in 0..len - 1 {
                for chunk in 0..num_chunks {
                    let index = 2 * (CUBE_DIM * chunk + ABSOLUTE_POS) + i % 2;
                    if index < len - 1 && input[index] > input[index + 1] {
                        let tmp = input[index];
                        input[index] = input[index + 1];
                        input[index + 1] = tmp;
                    }
                }
                sync_units();
            }
        }
    }
}

pub struct BitonicMergeSort;

#[cube]
impl Sort for BitonicMergeSort {
    fn sort<N: Numeric>(input: &mut Array<N>) {
        let num_chunks = f32::ceil(input.len() as f32 / CUBE_DIM as f32) as u32;
        let mut k = 2;
        while k <= input.len() {
            let mut j = k / 2;
            while j > 0 {
                for chunk in 0..num_chunks {
                    let i = chunk * CUBE_DIM + ABSOLUTE_POS;
                    let l = i ^ j;
                    if l > i
                        && l < input.len()
                        && ((i & k == 0) && input[i] > input[l]
                            || (i & k != 0) && input[i] < input[l])
                    {
                        let tmp = input[i];
                        input[i] = input[l];
                        input[l] = tmp;
                    }
                }
                sync_units();
                j /= 2;
            }
            k *= 2;
        }
    }
}

#[cube(launch_unchecked)]
fn sort_example<N: Numeric, S: Sort>(input: &mut Array<N>) {
    S::sort(input);
}

pub fn launch<R: Runtime>(device: &R::Device) {
    let client = R::client(device);

    let random_numbers = (0..2u32.pow(20))
        .map(|_| thread_rng().gen())
        .collect::<Vec<f32>>();
    let input = &random_numbers;

    let input_handle = client.create(f32::as_bytes(input));

    let time = Instant::now();
    unsafe {
        sort_example::launch_unchecked::<f32, BitonicMergeSort, R>(
            &client,
            CubeCount::Static(1, 1, 1),
            CubeDim::new(1024, 1, 1),
            ArrayArg::from_raw_parts(&input_handle, input.len(), 1),
        )
    };
    client.sync(cubecl::client::SyncType::Wait);
    let elapsed = time.elapsed();

    let bytes = client.read(input_handle.binding());
    let output = f32::from_bytes(&bytes);

    println!("Executed with runtime {:?} in {:?}", R::name(), elapsed);
    println!(
        "Sort succesful: {}\n {:?} {:?}",
        output.windows(2).all(|x| x[0] <= x[1])
            && (input.iter().sum::<f32>() - output.iter().sum::<f32>()).abs()
                / (input.len() as f32)
                < 0.001,
        false,
        false,
    );
}
