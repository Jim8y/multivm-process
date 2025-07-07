use std::collections::HashMap;

fn main() {
    // Simulate 3 validators
    let validators = vec\!["validator1", "validator2", "validator3"];
    
    // After sorting
    let mut sorted = validators.clone();
    sorted.sort();
    println\!("Sorted validators: {:?}", sorted);
    
    // Round-robin leader selection
    for round in 0..4 {
        let leader_index = round % sorted.len();
        println\!("Round {}: leader = {}", round, sorted[leader_index]);
    }
}
