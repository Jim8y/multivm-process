#[cfg(test)]
mod test {
    use multivm_consensus::view_change::*;
    use multivm_consensus::leader_selection::LeaderSelector;
    use multivm_consensus::malachite::types::{Round, ValidatorAddress};
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_simple_view_change() {
        let validators = vec\![
            ValidatorAddress("a".to_string()),
            ValidatorAddress("b".to_string()),
            ValidatorAddress("c".to_string()),
        ];

        let leader_selector = Arc::new(RwLock::new(
            LeaderSelector::new_round_robin(validators),
        ));

        let manager = ViewChangeManager::new(
            "a".to_string(),
            leader_selector.clone(),
            Duration::from_secs(30),
        );

        // Check initial leader
        let ls = leader_selector.read().await;
        println\!("Round 0 leader: {:?}", ls.get_leader(1, Round::new(0)));
        println\!("Round 1 leader: {:?}", ls.get_leader(1, Round::new(1)));
        println\!("Round 2 leader: {:?}", ls.get_leader(1, Round::new(2)));
        drop(ls);

        // Start view change to round 1
        let _ = manager.start_view_change(1, Round::new(1)).await.unwrap();
        
        // Get the leader
        let new_leader = manager.complete_view_change().await.unwrap();
        println\!("View change complete, new leader: {:?}", new_leader);
        
        assert_eq\!(new_leader, ValidatorAddress("b".to_string()));
    }
}

fn main() {}
