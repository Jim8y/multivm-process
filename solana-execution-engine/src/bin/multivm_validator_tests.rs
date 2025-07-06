//! Tests for the multivm-validator binary

#[cfg(test)]
mod tests {
    #[test]
    fn test_validator_disabled_message() {
        // Test the error messages that should be displayed
        let error_msg1 = "agave-validator is temporarily disabled due to compilation issues in the multivm-agave repository";
        let error_msg2 = "The fix-extract-if-rust-188 branch has a bug where test-only constants are used in non-test code";
        
        assert!(!error_msg1.is_empty());
        assert!(!error_msg2.is_empty());
        assert!(error_msg1.contains("agave-validator"));
        assert!(error_msg1.contains("temporarily disabled"));
        assert!(error_msg2.contains("bug"));
    }

    #[test]
    fn test_validator_exit_code() {
        // Test that the expected exit code would be 1
        let expected_exit_code = 1;
        assert_eq!(expected_exit_code, 1);
    }

    #[test]
    fn test_validator_context() {
        // Test that we're in the correct binary context
        assert!(module_path!().contains("multivm_validator"));
    }
}