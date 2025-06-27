//! Security Headers Middleware
//!
//! Implements comprehensive security headers to protect against common web vulnerabilities
//! including XSS, CSRF, clickjacking, and other security threats.

use axum::{
    extract::Request,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use std::collections::HashMap;
use tracing::{debug, warn};

/// Security headers configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Enable strict transport security
    pub enable_hsts: bool,
    /// HSTS max age in seconds
    pub hsts_max_age: u32,
    /// Include subdomains in HSTS
    pub hsts_include_subdomains: bool,
    /// Enable HSTS preload
    pub hsts_preload: bool,

    /// Content Security Policy
    pub csp_policy: Option<String>,
    /// Report CSP violations
    pub csp_report_only: bool,

    /// X-Frame-Options setting
    pub frame_options: FrameOptions,

    /// X-Content-Type-Options
    pub content_type_options: bool,

    /// Referrer Policy
    pub referrer_policy: ReferrerPolicy,

    /// Permissions Policy
    pub permissions_policy: Option<String>,

    /// Enable additional security headers
    pub enable_additional_headers: bool,

    /// Custom security headers
    pub custom_headers: HashMap<String, String>,

    /// Environment (affects header strictness)
    pub environment: Environment,
}

/// Frame options for X-Frame-Options header
#[derive(Debug, Clone)]
pub enum FrameOptions {
    /// Deny all framing
    Deny,
    /// Allow same origin framing
    SameOrigin,
    /// Allow specific URI framing
    AllowFrom(String),
    /// Disable X-Frame-Options (not recommended)
    Disabled,
}

/// Referrer policy options
#[derive(Debug, Clone)]
pub enum ReferrerPolicy {
    /// No referrer information
    NoReferrer,
    /// No referrer when downgrading HTTPS to HTTP
    NoReferrerWhenDowngrade,
    /// Origin only
    Origin,
    /// Origin when cross-origin
    OriginWhenCrossOrigin,
    /// Same origin only
    SameOrigin,
    /// Strict origin
    StrictOrigin,
    /// Strict origin when cross-origin
    StrictOriginWhenCrossOrigin,
    /// Unsafe URL (not recommended)
    UnsafeUrl,
}

/// Deployment environment
#[derive(Debug, Clone, PartialEq)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self::production()
    }
}

impl SecurityConfig {
    /// Create a production-ready security configuration
    pub fn production() -> Self {
        Self {
            enable_hsts: true,
            hsts_max_age: 31536000, // 1 year
            hsts_include_subdomains: true,
            hsts_preload: true,

            csp_policy: Some(Self::default_csp_policy()),
            csp_report_only: false,

            frame_options: FrameOptions::Deny,
            content_type_options: true,
            referrer_policy: ReferrerPolicy::StrictOriginWhenCrossOrigin,
            permissions_policy: Some(Self::default_permissions_policy()),
            enable_additional_headers: true,
            custom_headers: HashMap::new(),
            environment: Environment::Production,
        }
    }

    /// Create a development-friendly security configuration
    pub fn development() -> Self {
        Self {
            enable_hsts: false, // Don't enforce HTTPS in development
            hsts_max_age: 0,
            hsts_include_subdomains: false,
            hsts_preload: false,

            csp_policy: Some(Self::relaxed_csp_policy()),
            csp_report_only: true, // Report only in development

            frame_options: FrameOptions::SameOrigin,
            content_type_options: true,
            referrer_policy: ReferrerPolicy::OriginWhenCrossOrigin,
            permissions_policy: Some(Self::default_permissions_policy()),
            enable_additional_headers: true,
            custom_headers: HashMap::new(),
            environment: Environment::Development,
        }
    }

    /// Default Content Security Policy for production
    fn default_csp_policy() -> String {
        [
            "default-src 'self'",
            "script-src 'self' 'unsafe-inline' 'unsafe-eval'", // Needed for some frameworks
            "style-src 'self' 'unsafe-inline'",                // Needed for inline styles
            "img-src 'self' data: https:",
            "font-src 'self' data: https:",
            "connect-src 'self' wss: https:",
            "frame-src 'none'",
            "object-src 'none'",
            "base-uri 'self'",
            "form-action 'self'",
            "frame-ancestors 'none'",
            "upgrade-insecure-requests",
        ]
        .join("; ")
    }

    /// Relaxed Content Security Policy for development
    fn relaxed_csp_policy() -> String {
        [
            "default-src 'self' 'unsafe-inline' 'unsafe-eval'",
            "script-src 'self' 'unsafe-inline' 'unsafe-eval' localhost:* 127.0.0.1:*",
            "style-src 'self' 'unsafe-inline' localhost:* 127.0.0.1:*",
            "img-src 'self' data: https: http:",
            "font-src 'self' data: https: http:",
            "connect-src 'self' ws: wss: http: https: localhost:* 127.0.0.1:*",
            "frame-src 'self'",
            "object-src 'none'",
            "base-uri 'self'",
            "form-action 'self'",
        ]
        .join("; ")
    }

    /// Default Permissions Policy
    fn default_permissions_policy() -> String {
        [
            "camera=()",
            "microphone=()",
            "geolocation=()",
            "gyroscope=()",
            "magnetometer=()",
            "payment=()",
            "usb=()",
        ]
        .join(", ")
    }

    /// Add a custom header
    pub fn add_custom_header(&mut self, name: String, value: String) {
        self.custom_headers.insert(name, value);
    }

    /// Set CSP policy
    pub fn set_csp_policy(&mut self, policy: String) {
        self.csp_policy = Some(policy);
    }

    /// Enable/disable HSTS
    pub fn set_hsts(&mut self, enabled: bool, max_age: u32) {
        self.enable_hsts = enabled;
        self.hsts_max_age = max_age;
    }
}

/// Security headers middleware
pub async fn security_headers_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    security_headers_with_config(request, next, &SecurityConfig::default()).await
}

/// Security headers middleware with custom configuration
pub async fn security_headers_with_config(
    request: Request,
    next: Next,
    config: &SecurityConfig,
) -> Result<Response, StatusCode> {
    let mut response = next.run(request).await;

    // Add security headers to response
    apply_security_headers(response.headers_mut(), config);

    Ok(response)
}

/// Apply security headers to a response
pub fn apply_security_headers(headers: &mut HeaderMap, config: &SecurityConfig) {
    // Strict Transport Security (HSTS)
    if config.enable_hsts {
        let mut hsts_value = format!("max-age={}", config.hsts_max_age);

        if config.hsts_include_subdomains {
            hsts_value.push_str("; includeSubDomains");
        }

        if config.hsts_preload {
            hsts_value.push_str("; preload");
        }

        add_header_safe(headers, "Strict-Transport-Security", &hsts_value);
    }

    // Content Security Policy
    if let Some(ref csp_policy) = config.csp_policy {
        let header_name = if config.csp_report_only {
            "Content-Security-Policy-Report-Only"
        } else {
            "Content-Security-Policy"
        };
        add_header_safe(headers, header_name, csp_policy);
    }

    // X-Frame-Options
    match config.frame_options {
        FrameOptions::Deny => add_header_safe(headers, "X-Frame-Options", "DENY"),
        FrameOptions::SameOrigin => add_header_safe(headers, "X-Frame-Options", "SAMEORIGIN"),
        FrameOptions::AllowFrom(ref uri) => {
            add_header_safe(headers, "X-Frame-Options", &format!("ALLOW-FROM {uri}"));
        }
        FrameOptions::Disabled => {} // Don't add the header
    }

    // X-Content-Type-Options
    if config.content_type_options {
        add_header_safe(headers, "X-Content-Type-Options", "nosniff");
    }

    // Referrer Policy
    let referrer_value = match config.referrer_policy {
        ReferrerPolicy::NoReferrer => "no-referrer",
        ReferrerPolicy::NoReferrerWhenDowngrade => "no-referrer-when-downgrade",
        ReferrerPolicy::Origin => "origin",
        ReferrerPolicy::OriginWhenCrossOrigin => "origin-when-cross-origin",
        ReferrerPolicy::SameOrigin => "same-origin",
        ReferrerPolicy::StrictOrigin => "strict-origin",
        ReferrerPolicy::StrictOriginWhenCrossOrigin => "strict-origin-when-cross-origin",
        ReferrerPolicy::UnsafeUrl => "unsafe-url",
    };
    add_header_safe(headers, "Referrer-Policy", referrer_value);

    // Permissions Policy
    if let Some(ref permissions) = config.permissions_policy {
        add_header_safe(headers, "Permissions-Policy", permissions);
    }

    // Additional security headers
    if config.enable_additional_headers {
        // X-XSS-Protection (legacy, but still useful for older browsers)
        add_header_safe(headers, "X-XSS-Protection", "1; mode=block");

        // X-DNS-Prefetch-Control
        add_header_safe(headers, "X-DNS-Prefetch-Control", "off");

        // X-Download-Options (IE-specific)
        add_header_safe(headers, "X-Download-Options", "noopen");

        // X-Permitted-Cross-Domain-Policies
        add_header_safe(headers, "X-Permitted-Cross-Domain-Policies", "none");

        // Cross-Origin-Embedder-Policy
        add_header_safe(headers, "Cross-Origin-Embedder-Policy", "require-corp");

        // Cross-Origin-Opener-Policy
        add_header_safe(headers, "Cross-Origin-Opener-Policy", "same-origin");

        // Cross-Origin-Resource-Policy
        add_header_safe(headers, "Cross-Origin-Resource-Policy", "same-origin");
    }

    // Custom headers
    for (name, value) in &config.custom_headers {
        add_header_safe(headers, name, value);
    }

    // Environment-specific headers
    match config.environment {
        Environment::Development => {
            add_header_safe(headers, "X-Environment", "development");
        }
        Environment::Staging => {
            add_header_safe(headers, "X-Environment", "staging");
        }
        Environment::Production => {
            // Don't expose environment in production
        }
    }
}

/// Safely add a header to the response
fn add_header_safe(headers: &mut HeaderMap, name: &str, value: &str) {
    match (HeaderName::try_from(name), HeaderValue::try_from(value)) {
        (Ok(header_name), Ok(header_value)) => {
            headers.insert(header_name, header_value);
            debug!("Added security header: {} = {}", name, value);
        }
        (Err(e), _) => {
            warn!("Invalid header name '{}': {}", name, e);
        }
        (_, Err(e)) => {
            warn!("Invalid header value for '{}': {}", name, e);
        }
    }
}

/// Content Security Policy builder
pub struct CspBuilder {
    directives: HashMap<String, Vec<String>>,
}

impl CspBuilder {
    /// Create a new CSP builder
    pub fn new() -> Self {
        Self {
            directives: HashMap::new(),
        }
    }

    /// Add a directive with sources
    pub fn directive(&mut self, directive: &str, sources: Vec<&str>) -> &mut Self {
        self.directives.insert(
            directive.to_string(),
            sources.into_iter().map(|s| s.to_string()).collect(),
        );
        self
    }

    /// Add default-src directive
    pub fn default_src(&mut self, sources: Vec<&str>) -> &mut Self {
        self.directive("default-src", sources)
    }

    /// Add script-src directive
    pub fn script_src(&mut self, sources: Vec<&str>) -> &mut Self {
        self.directive("script-src", sources)
    }

    /// Add style-src directive
    pub fn style_src(&mut self, sources: Vec<&str>) -> &mut Self {
        self.directive("style-src", sources)
    }

    /// Add img-src directive
    pub fn img_src(&mut self, sources: Vec<&str>) -> &mut Self {
        self.directive("img-src", sources)
    }

    /// Add connect-src directive
    pub fn connect_src(&mut self, sources: Vec<&str>) -> &mut Self {
        self.directive("connect-src", sources)
    }

    /// Add frame-src directive
    pub fn frame_src(&mut self, sources: Vec<&str>) -> &mut Self {
        self.directive("frame-src", sources)
    }

    /// Build the CSP policy string
    pub fn build(&self) -> String {
        let mut policy_parts = Vec::new();

        for (directive, sources) in &self.directives {
            let directive_str = format!("{} {}", directive, sources.join(" "));
            policy_parts.push(directive_str);
        }

        policy_parts.join("; ")
    }
}

impl Default for CspBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Security audit utility
pub struct SecurityAudit;

impl SecurityAudit {
    /// Audit security headers in a response
    pub fn audit_headers(headers: &HeaderMap) -> SecurityAuditReport {
        let mut report = SecurityAuditReport::new();

        // Check for required security headers
        let required_headers = [
            "Content-Security-Policy",
            "X-Frame-Options",
            "X-Content-Type-Options",
            "Referrer-Policy",
        ];

        for header in required_headers {
            if headers.contains_key(header) {
                report.add_pass(format!("{header} header present"));
            } else {
                report.add_warning(format!("{header} header missing"));
            }
        }

        // Check HSTS for HTTPS responses
        if !headers.contains_key("Strict-Transport-Security") {
            report.add_warning("HSTS header missing (should be present for HTTPS)".to_string());
        }

        // Check for insecure CSP policies
        if let Some(csp) = headers.get("Content-Security-Policy") {
            if let Ok(csp_str) = csp.to_str() {
                if csp_str.contains("'unsafe-eval'") {
                    report.add_warning("CSP contains 'unsafe-eval' directive".to_string());
                }
                if csp_str.contains("*") {
                    report.add_warning("CSP contains wildcard (*) directive".to_string());
                }
            }
        }

        report
    }
}

/// Security audit report
#[derive(Debug, Clone)]
pub struct SecurityAuditReport {
    pub passes: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl SecurityAuditReport {
    fn new() -> Self {
        Self {
            passes: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn add_pass(&mut self, message: String) {
        self.passes.push(message);
    }

    fn add_warning(&mut self, message: String) {
        self.warnings.push(message);
    }

    fn _add_error(&mut self, message: String) {
        self.errors.push(message);
    }

    /// Get the overall security score (0-100)
    pub fn security_score(&self) -> u8 {
        let total_checks = self.passes.len() + self.warnings.len() + self.errors.len();
        if total_checks == 0 {
            return 100;
        }

        let score = (self.passes.len() as f64 / total_checks as f64) * 100.0;
        score as u8
    }

    /// Check if the security configuration is considered secure
    pub fn is_secure(&self) -> bool {
        self.errors.is_empty() && self.warnings.len() <= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csp_builder() {
        let policy = CspBuilder::new()
            .default_src(vec!["'self'"])
            .script_src(vec!["'self'", "'unsafe-inline'"])
            .style_src(vec!["'self'", "'unsafe-inline'"])
            .build();

        assert!(policy.contains("default-src 'self'"));
        assert!(policy.contains("script-src 'self' 'unsafe-inline'"));
        assert!(policy.contains("style-src 'self' 'unsafe-inline'"));
    }

    #[test]
    fn test_security_config_production() {
        let config = SecurityConfig::production();
        assert!(config.enable_hsts);
        assert!(config.content_type_options);
        assert!(config.csp_policy.is_some());
        assert_eq!(config.environment, Environment::Production);
    }

    #[test]
    fn test_security_config_development() {
        let config = SecurityConfig::development();
        assert!(!config.enable_hsts);
        assert!(config.csp_report_only);
        assert_eq!(config.environment, Environment::Development);
    }

    #[test]
    fn test_security_audit() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Content-Security-Policy",
            "default-src 'self'".parse().unwrap(),
        );
        headers.insert("X-Frame-Options", "DENY".parse().unwrap());

        let report = SecurityAudit::audit_headers(&headers);
        assert!(!report.passes.is_empty());
        assert!(report.security_score() > 0);
    }
}
