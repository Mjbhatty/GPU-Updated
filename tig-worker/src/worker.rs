use anyhow::{anyhow, Result};
use bincode;
use tig_challenges::*;
pub use tig_structs::core::{BenchmarkSettings, Solution, SolutionData};
use tig_utils::decompress_obj;
use wasmi::{Config, Engine, Linker, Module, Store, StoreLimitsBuilder};

pub fn compute_solution(
    settings: &BenchmarkSettings,
    nonce: u32,
    wasm: &[u8],
    max_memory: u64,
    max_fuel: u64,
) -> Result<Option<SolutionData>> {
    let seed = settings.calc_seed(nonce);
    let serialized_challenge = match settings.challenge_id.as_str() {
        "c001" => {
            let challenge =
                satisfiability::Challenge::generate_instance_from_vec(seed, &settings.difficulty)
                    .unwrap();
            bincode::serialize(&challenge).unwrap()
        }
        "c002" => {
            let challenge =
                vehicle_routing::Challenge::generate_instance_from_vec(seed, &settings.difficulty)
                    .unwrap();
            bincode::serialize(&challenge).unwrap()
        }
        "c003" => {
            let challenge =
                knapsack::Challenge::generate_instance_from_vec(seed, &settings.difficulty)
                    .unwrap();
            bincode::serialize(&challenge).unwrap()
        }
        "c004" => {
            let challenge =
                vector_search::Challenge::generate_instance_from_vec(seed, &settings.difficulty)
                    .unwrap();
            bincode::serialize(&challenge).unwrap()
        }
        _ => panic!("Unknown challenge"),
    };

    let mut config = Config::default();
    config.update_runtime_signature(true);
    config.consume_fuel(true);

    let limits = StoreLimitsBuilder::new()
        .memory_size(max_memory as usize)
        .memories(1)
        .trap_on_grow_failure(true)
        .build();
    // Setup instance of wasm module
    let engine = Engine::new(&config);
    let mut store = Store::new(&engine, limits);
    store.limiter(|lim| lim);
    store.set_fuel(max_fuel).unwrap();
    let linker = Linker::new(&engine);
    let module = Module::new(store.engine(), wasm).expect("Failed to instantiate module");

    let instance = &linker
        .instantiate(&mut store, &module)
        .expect("Failed to instantiate linker")
        .start(&mut store)
        .expect("Failed to start module");

    let memory = instance
        .get_memory(&store, "memory")
        .expect("Failed to find memory");

    // Run algorithm
    let init = instance
        .get_typed_func::<u32, u32>(&store, "init")
        .expect("Failed to find `init` function");
    let entry_point = instance
        .get_typed_func::<(u32, u32), u32>(&store, "entry_point")
        .expect("Failed to find `entry_point` function");

    let challenge_len = serialized_challenge.len() as u32;
    let challenge_ptr: u32 = init.call(&mut store, challenge_len).unwrap();
    memory
        .write(&mut store, challenge_ptr as usize, &serialized_challenge)
        .expect("Failed to write serialized challenge to `memory`");
    let solution_ptr = entry_point
        .call(&mut store, (challenge_ptr, challenge_len))
        .map_err(|e| anyhow!("Failed to call function: {:?}", e))?;

    // Get runtime signature
    let runtime_signature_u64 = store.get_runtime_signature();
    let runtime_signature = (runtime_signature_u64 as u32) ^ ((runtime_signature_u64 >> 32) as u32);
    let fuel_consumed = store.get_fuel().unwrap();
    // Read solution from memory
    let mut solution_len_bytes = [0u8; 4];
    memory
        .read(&store, solution_ptr as usize, &mut solution_len_bytes)
        .expect("Failed to read solution length from memory");
    let solution_len = u32::from_le_bytes(solution_len_bytes);
    let mut serialized_solution = vec![0u8; solution_len as usize];
    memory
        .read(
            &store,
            (solution_ptr + 4) as usize,
            &mut serialized_solution,
        )
        .expect("Failed to read solution from memory");
    let mut solution_data = SolutionData {
        nonce,
        runtime_signature,
        fuel_consumed,
        solution: Solution::new(),
    };
    if solution_len != 0 {
        solution_data.solution =
            decompress_obj(&serialized_solution).expect("Failed to decompress solution");
    }
    Ok(Some(solution_data))
}

pub fn verify_solution(
    settings: &BenchmarkSettings,
    nonce: u32,
    solution: &Solution,
) -> Result<()> {
    let seed = settings.calc_seed(nonce);
    match settings.challenge_id.as_str() {
        "c001" => {
            let challenge =
                satisfiability::Challenge::generate_instance_from_vec(seed, &settings.difficulty)
                    .expect("Failed to generate satisfiability instance");
            match satisfiability::Solution::try_from(solution.clone()) {
                Ok(solution) => challenge.verify_solution(&solution),
                Err(_) => Err(anyhow!(
                    "Invalid solution. Cannot convert to satisfiability::Solution"
                )),
            }
        }
        "c002" => {
            let challenge =
                vehicle_routing::Challenge::generate_instance_from_vec(seed, &settings.difficulty)
                    .expect("Failed to generate vehicle_routing instance");
            match vehicle_routing::Solution::try_from(solution.clone()) {
                Ok(solution) => challenge.verify_solution(&solution),
                Err(_) => Err(anyhow!(
                    "Invalid solution. Cannot convert to vehicle_routing::Solution"
                )),
            }
        }
        "c003" => {
            let challenge =
                knapsack::Challenge::generate_instance_from_vec(seed, &settings.difficulty)
                    .expect("Failed to generate knapsack instance");
            match knapsack::Solution::try_from(solution.clone()) {
                Ok(solution) => challenge.verify_solution(&solution),
                Err(_) => Err(anyhow!(
                    "Invalid solution. Cannot convert to knapsack::Solution"
                )),
            }
        }
        "c004" => {
            let challenge =
                vector_search::Challenge::generate_instance_from_vec(seed, &settings.difficulty)
                    .expect("Failed to generate vector_search instance");
            match vector_search::Solution::try_from(solution.clone()) {
                Ok(solution) => challenge.verify_solution(&solution),
                Err(_) => Err(anyhow!(
                    "Invalid solution. Cannot convert to vector_search::Solution"
                )),
            }
        }
        _ => panic!("Unknown challenge"),
    }
}