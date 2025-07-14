#!/bin/bash

# JWT Token Generator for Reth Engine API
# Generates JWT tokens for authenticated communication with Reth's Engine API

set -euo pipefail

# Configuration
JWT_EXPIRY_SECONDS="${JWT_EXPIRY_SECONDS:-300}"  # 5 minutes default

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Help function
show_help() {
    cat << EOF
JWT Token Generator for Reth Engine API

USAGE:
    $0 [SECRET] [OPTIONS]

ARGUMENTS:
    SECRET              JWT secret (64 hex characters) or path to secret file

OPTIONS:
    --expiry SECONDS    Token expiry time in seconds (default: ${JWT_EXPIRY_SECONDS})
    --quiet             Only output the token
    --verify TOKEN      Verify an existing token instead of generating
    --help              Show this help message

EXAMPLES:
    # Generate token from secret file
    $0 ./reth-data/jwt.hex

    # Generate token from direct secret
    $0 1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef

    # Generate with custom expiry (1 hour)
    $0 ./reth-data/jwt.hex --expiry 3600

    # Quiet mode (only output token)
    $0 ./reth-data/jwt.hex --quiet

    # Verify existing token
    $0 ./reth-data/jwt.hex --verify eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...

ENVIRONMENT VARIABLES:
    JWT_EXPIRY_SECONDS  Default token expiry time (default: 300)
EOF
}

# Base64 URL-safe encoding without padding
base64url_encode() {
    local input="$1"
    echo -n "$input" | base64 -w 0 | tr '+/' '-_' | tr -d '='
}

# Base64 URL-safe decoding
base64url_decode() {
    local input="$1"
    # Add padding if needed
    local padded="$input"
    case $((${#input} % 4)) in
        2) padded="${input}==" ;;
        3) padded="${input}=" ;;
    esac
    echo -n "$padded" | tr '_-' '/+' | base64 -d
}

# Generate HMAC-SHA256 signature
hmac_sha256() {
    local secret="$1"
    local data="$2"
    
    # Convert hex secret to binary
    local binary_secret=$(echo -n "$secret" | xxd -r -p)
    echo -n "$data" | openssl dgst -sha256 -hmac "$binary_secret" -binary | base64 -w 0 | tr '+/' '-_' | tr -d '='
}

# Validate hex secret
validate_secret() {
    local secret="$1"
    
    # Check if it's exactly 64 hex characters
    if [[ ! "$secret" =~ ^[0-9a-fA-F]{64}$ ]]; then
        log_error "Invalid JWT secret format"
        log_error "Expected: 64 hexadecimal characters (256-bit key)"
        log_error "Got: $secret"
        return 1
    fi
    
    return 0
}

# Load secret from file or use direct input
load_secret() {
    local input="$1"
    
    if [[ -f "$input" ]]; then
        # It's a file path
        if [[ ! -r "$input" ]]; then
            log_error "Cannot read secret file: $input"
            return 1
        fi
        
        local secret=$(cat "$input" | tr -d '\n\r\t ')
        validate_secret "$secret" || return 1
        echo "$secret"
    else
        # It's a direct secret
        validate_secret "$input" || return 1
        echo "$input"
    fi
}

# Generate JWT token
generate_jwt() {
    local secret="$1"
    local expiry_seconds="$2"
    local quiet="$3"
    
    # Current timestamp
    local now=$(date +%s)
    local exp=$((now + expiry_seconds))
    
    # JWT Header
    local header='{"alg":"HS256","typ":"JWT"}'
    local header_b64=$(base64url_encode "$header")
    
    # JWT Payload
    local payload="{\"iat\":$now,\"exp\":$exp}"
    local payload_b64=$(base64url_encode "$payload")
    
    # Data to sign
    local data="${header_b64}.${payload_b64}"
    
    # Generate signature
    local signature=$(hmac_sha256 "$secret" "$data")
    
    # Complete JWT token
    local token="${data}.${signature}"
    
    if [[ "$quiet" != "true" ]]; then
        log_success "JWT token generated successfully"
        log_info "Issued at: $(date -d "@$now" '+%Y-%m-%d %H:%M:%S %Z')"
        log_info "Expires at: $(date -d "@$exp" '+%Y-%m-%d %H:%M:%S %Z')"
        log_info "Valid for: ${expiry_seconds} seconds"
        echo ""
        echo "JWT Token:"
    fi
    
    echo "$token"
}

# Verify JWT token
verify_jwt() {
    local secret="$1"
    local token="$2"
    local quiet="$3"
    
    # Split token into parts
    IFS='.' read -ra parts <<< "$token"
    
    if [[ ${#parts[@]} -ne 3 ]]; then
        log_error "Invalid JWT format: expected 3 parts separated by dots"
        return 1
    fi
    
    local header_b64="${parts[0]}"
    local payload_b64="${parts[1]}"
    local signature="${parts[2]}"
    
    # Decode header and payload
    local header
    local payload
    
    if ! header=$(base64url_decode "$header_b64" 2>/dev/null); then
        log_error "Failed to decode JWT header"
        return 1
    fi
    
    if ! payload=$(base64url_decode "$payload_b64" 2>/dev/null); then
        log_error "Failed to decode JWT payload"
        return 1
    fi
    
    # Verify algorithm
    local alg=$(echo "$header" | python3 -c "import sys, json; print(json.load(sys.stdin).get('alg', ''))" 2>/dev/null || echo "")
    if [[ "$alg" != "HS256" ]]; then
        log_error "Unsupported algorithm: $alg (expected HS256)"
        return 1
    fi
    
    # Verify signature
    local data="${header_b64}.${payload_b64}"
    local expected_signature=$(hmac_sha256 "$secret" "$data")
    
    if [[ "$signature" != "$expected_signature" ]]; then
        log_error "Invalid signature"
        if [[ "$quiet" != "true" ]]; then
            log_error "Expected: $expected_signature"
            log_error "Got:      $signature"
        fi
        return 1
    fi
    
    # Extract expiry time
    local exp=$(echo "$payload" | python3 -c "import sys, json; print(json.load(sys.stdin).get('exp', 0))" 2>/dev/null || echo "0")
    local iat=$(echo "$payload" | python3 -c "import sys, json; print(json.load(sys.stdin).get('iat', 0))" 2>/dev/null || echo "0")
    local now=$(date +%s)
    
    # Check expiry
    if [[ $exp -le $now ]]; then
        log_error "Token has expired"
        if [[ "$quiet" != "true" ]]; then
            log_error "Expired at: $(date -d "@$exp" '+%Y-%m-%d %H:%M:%S %Z')"
            log_error "Current time: $(date -d "@$now" '+%Y-%m-%d %H:%M:%S %Z')"
        fi
        return 1
    fi
    
    if [[ "$quiet" != "true" ]]; then
        log_success "JWT token is valid"
        log_info "Issued at: $(date -d "@$iat" '+%Y-%m-%d %H:%M:%S %Z')"
        log_info "Expires at: $(date -d "@$exp" '+%Y-%m-%d %H:%M:%S %Z')"
        log_info "Time remaining: $((exp - now)) seconds"
        echo ""
        echo "Header:"
        echo "$header" | python3 -m json.tool 2>/dev/null || echo "$header"
        echo ""
        echo "Payload:"
        echo "$payload" | python3 -m json.tool 2>/dev/null || echo "$payload"
    fi
    
    return 0
}

# Check dependencies
check_dependencies() {
    local missing_deps=()
    
    if ! command -v openssl &> /dev/null; then
        missing_deps+=("openssl")
    fi
    
    if ! command -v base64 &> /dev/null; then
        missing_deps+=("base64")
    fi
    
    if ! command -v xxd &> /dev/null; then
        missing_deps+=("xxd")
    fi
    
    if ! command -v python3 &> /dev/null; then
        missing_deps+=("python3")
    fi
    
    if [[ ${#missing_deps[@]} -gt 0 ]]; then
        log_error "Missing required dependencies: ${missing_deps[*]}"
        log_error "Please install the missing tools and try again"
        return 1
    fi
    
    return 0
}

# Main function
main() {
    local secret_input=""
    local expiry_seconds="$JWT_EXPIRY_SECONDS"
    local quiet="false"
    local verify_mode="false"
    local verify_token=""
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --expiry)
                expiry_seconds="$2"
                shift 2
                ;;
            --quiet|-q)
                quiet="true"
                shift
                ;;
            --verify)
                verify_mode="true"
                verify_token="$2"
                shift 2
                ;;
            --help|-h)
                show_help
                exit 0
                ;;
            -*)
                log_error "Unknown option: $1"
                echo "Use --help for usage information"
                exit 1
                ;;
            *)
                if [[ -z "$secret_input" ]]; then
                    secret_input="$1"
                else
                    log_error "Too many arguments"
                    echo "Use --help for usage information"
                    exit 1
                fi
                shift
                ;;
        esac
    done
    
    # Check for required argument
    if [[ -z "$secret_input" ]]; then
        log_error "Missing required argument: SECRET"
        echo "Use --help for usage information"
        exit 1
    fi
    
    # Check dependencies
    check_dependencies || exit 1
    
    # Load and validate secret
    local secret
    if ! secret=$(load_secret "$secret_input"); then
        exit 1
    fi
    
    # Validate expiry
    if ! [[ "$expiry_seconds" =~ ^[0-9]+$ ]] || [[ $expiry_seconds -le 0 ]]; then
        log_error "Invalid expiry time: $expiry_seconds (must be positive integer)"
        exit 1
    fi
    
    # Generate or verify token
    if [[ "$verify_mode" == "true" ]]; then
        verify_jwt "$secret" "$verify_token" "$quiet"
    else
        generate_jwt "$secret" "$expiry_seconds" "$quiet"
    fi
}

# Run main function with all arguments
main "$@"