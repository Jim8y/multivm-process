//! Tests for error handling

#[cfg(test)]
mod tests {
    use crate::{Error, Result, error::ErrorContext};
    use std::io;

    #[test]
    fn test_error_conversions() {
        // IO error conversion
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));

        // Serde JSON error conversion
        let json_err = serde_json::from_str::<String>("invalid").unwrap_err();
        let err: Error = json_err.into();
        assert!(matches!(err, Error::Serialization(_)));

        // Timeout error is tested separately since we can't create it directly

        // Address parse error conversion
        let addr_err = "invalid:address".parse::<std::net::SocketAddr>().unwrap_err();
        let err: Error = addr_err.into();
        assert!(matches!(err, Error::InvalidInput(_)));
    }

    #[test]
    fn test_error_context() {

        // Test context on Result
        let result: Result<i32> = Err(Error::NotFound("test".to_string()));
        let with_context = result.context("Looking for resource");
        assert!(with_context.is_err());
        match with_context.unwrap_err() {
            Error::Other(msg) => assert!(msg.contains("Looking for resource")),
            _ => panic!("Expected Other error"),
        }

        // Test with_context closure
        let result: Result<i32> = Err(Error::InvalidState("bad state".to_string()));
        let with_context = result.with_context(|| format!("Processing item {}", 42));
        assert!(with_context.is_err());
        match with_context.unwrap_err() {
            Error::Other(msg) => assert!(msg.contains("Processing item 42")),
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_error_display() {
        let errors = vec![
            Error::Io(io::Error::new(io::ErrorKind::PermissionDenied, "access denied")),
            Error::Serialization("invalid JSON".to_string()),
            Error::Network("connection refused".to_string()),
            Error::Storage("disk full".to_string()),
            Error::InvalidConfig("missing field".to_string()),
            Error::NotFound("resource X".to_string()),
            Error::Timeout,
            Error::PermissionDenied("admin only".to_string()),
            Error::ResourceExhausted("out of memory".to_string()),
            Error::InvalidState("not ready".to_string()),
            Error::InvalidInput("negative value".to_string()),
            Error::Other("unknown error".to_string()),
        ];

        for err in errors {
            let display = format!("{}", err);
            assert!(!display.is_empty());
            
            // Test Debug implementation
            let debug = format!("{:?}", err);
            assert!(!debug.is_empty());
        }
    }

    #[test]
    fn test_error_chaining() {
        fn inner_operation() -> Result<()> {
            Err(Error::NotFound("config file".to_string()))
        }

        fn middle_operation() -> Result<()> {
            inner_operation().context("Loading configuration")
        }

        fn outer_operation() -> Result<()> {
            middle_operation().context("Starting application")
        }

        let result = outer_operation();
        assert!(result.is_err());
        
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Starting application"));
        assert!(err_msg.contains("Loading configuration"));
    }

    #[test]
    fn test_result_type_alias() {
        fn success_function() -> Result<String> {
            Ok("success".to_string())
        }

        fn failure_function() -> Result<String> {
            Err(Error::Other("failed".to_string()))
        }

        assert!(success_function().is_ok());
        assert!(failure_function().is_err());
    }
}