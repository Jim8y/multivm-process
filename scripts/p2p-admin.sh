#!/bin/bash

# P2P Network Administration CLI
# Provides command-line interface for managing P2P network operations

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Default configuration
DEFAULT_API_URL="http://localhost:8080/api/v1"
DEFAULT_AUTH_TOKEN="admin-token"

# Configuration
API_URL="${P2P_API_URL:-$DEFAULT_API_URL}"
AUTH_TOKEN="${P2P_AUTH_TOKEN:-$DEFAULT_AUTH_TOKEN}"

function show_help() {
    echo -e "${BLUE}MultiVM P2P Network Administration CLI${NC}"
    echo ""
    echo "Usage: $0 [command] [options]"
    echo ""
    echo "Commands:"
    echo "  status                    Show network status"
    echo "  peers list               List all connected peers"
    echo "  peers info <peer_id>     Get detailed peer information"
    echo "  peers connect <address>  Connect to a new peer"
    echo "  peers disconnect <id>    Disconnect from a peer"
    echo "  ban <ip> [duration]      Ban an IP address"
    echo "  unban <ip>               Unban an IP address"
    echo "  banned                   List banned IP addresses"
    echo "  ratelimit                Show rate limiting status"
    echo "  ratelimit reset <peer>   Reset rate limit for a peer"
    echo "  health                   Get network health report"
    echo "  emergency on|off         Enable/disable emergency mode"
    echo "  metrics                  Show network metrics"
    echo "  monitor                  Real-time network monitoring"
    echo "  alerts                   Show active alerts"
    echo "  logs [level]             Show recent logs or set log level"
    echo ""
    echo "Options:"
    echo "  -h, --help              Show this help message"
    echo "  -u, --url <url>         API URL (default: $DEFAULT_API_URL)"
    echo "  -t, --token <token>     Auth token"
    echo "  -v, --verbose           Enable verbose output"
    echo "  -j, --json              Output in JSON format"
    echo ""
    echo "Environment Variables:"
    echo "  P2P_API_URL             API endpoint URL"
    echo "  P2P_AUTH_TOKEN          Authentication token"
}

function api_call() {
    local endpoint="$1"
    local method="${2:-GET}"
    local data="$3"
    
    local curl_args=(-s -X "$method" -H "Authorization: Bearer $AUTH_TOKEN")
    
    if [[ -n "$data" ]]; then
        curl_args+=(-H "Content-Type: application/json" -d "$data")
    fi
    
    curl "${curl_args[@]}" "$API_URL$endpoint"
}

function format_output() {
    local json_data="$1"
    
    if [[ "$JSON_OUTPUT" == "true" ]]; then
        echo "$json_data" | jq .
    else
        echo "$json_data" | jq -r .
    fi
}

function show_status() {
    echo -e "${BLUE}Network Status${NC}"
    echo "==============="
    
    local response
    response=$(api_call "/p2p/status")
    
    if [[ $? -eq 0 ]]; then
        local peer_count health_status uptime
        peer_count=$(echo "$response" | jq -r '.peer_count // "N/A"')
        health_status=$(echo "$response" | jq -r '.health_status // "Unknown"')
        uptime=$(echo "$response" | jq -r '.uptime // "N/A"')
        
        echo -e "Peer Count:     ${GREEN}$peer_count${NC}"
        echo -e "Health Status:  ${GREEN}$health_status${NC}"
        echo -e "Uptime:         ${CYAN}$uptime${NC}"
    else
        echo -e "${RED}Failed to get network status${NC}"
        exit 1
    fi
}

function list_peers() {
    echo -e "${BLUE}Connected Peers${NC}"
    echo "==============="
    
    local response
    response=$(api_call "/p2p/peers")
    
    if [[ $? -eq 0 ]]; then
        local peer_count
        peer_count=$(echo "$response" | jq '. | length')
        
        if [[ "$peer_count" -eq 0 ]]; then
            echo "No peers connected"
        else
            echo "$response" | jq -r '.[] | "\(.peer_id) - \(.status) - \(.last_seen)"'
        fi
    else
        echo -e "${RED}Failed to get peer list${NC}"
        exit 1
    fi
}

function get_peer_info() {
    local peer_id="$1"
    
    if [[ -z "$peer_id" ]]; then
        echo -e "${RED}Error: Peer ID required${NC}"
        exit 1
    fi
    
    echo -e "${BLUE}Peer Information: $peer_id${NC}"
    echo "=============================="
    
    local response
    response=$(api_call "/p2p/peers/$peer_id")
    
    if [[ $? -eq 0 ]]; then
        format_output "$response"
    else
        echo -e "${RED}Failed to get peer information${NC}"
        exit 1
    fi
}

function connect_peer() {
    local address="$1"
    
    if [[ -z "$address" ]]; then
        echo -e "${RED}Error: Peer address required${NC}"
        exit 1
    fi
    
    echo -e "${BLUE}Connecting to peer: $address${NC}"
    
    local data="{\"address\": \"$address\"}"
    local response
    response=$(api_call "/p2p/peers/connect" "POST" "$data")
    
    if [[ $? -eq 0 ]]; then
        echo -e "${GREEN}Connection initiated${NC}"
    else
        echo -e "${RED}Failed to connect to peer${NC}"
        exit 1
    fi
}

function disconnect_peer() {
    local peer_id="$1"
    
    if [[ -z "$peer_id" ]]; then
        echo -e "${RED}Error: Peer ID required${NC}"
        exit 1
    fi
    
    echo -e "${BLUE}Disconnecting from peer: $peer_id${NC}"
    
    local response
    response=$(api_call "/p2p/peers/$peer_id/disconnect" "POST")
    
    if [[ $? -eq 0 ]]; then
        echo -e "${GREEN}Peer disconnected${NC}"
    else
        echo -e "${RED}Failed to disconnect peer${NC}"
        exit 1
    fi
}

function ban_ip() {
    local ip="$1"
    local duration="$2"
    
    if [[ -z "$ip" ]]; then
        echo -e "${RED}Error: IP address required${NC}"
        exit 1
    fi
    
    echo -e "${BLUE}Banning IP: $ip${NC}"
    
    local data="{\"ip\": \"$ip\""
    if [[ -n "$duration" ]]; then
        data="$data, \"duration\": \"$duration\""
    fi
    data="$data}"
    
    local response
    response=$(api_call "/p2p/ban" "POST" "$data")
    
    if [[ $? -eq 0 ]]; then
        echo -e "${GREEN}IP banned${NC}"
    else
        echo -e "${RED}Failed to ban IP${NC}"
        exit 1
    fi
}

function unban_ip() {
    local ip="$1"
    
    if [[ -z "$ip" ]]; then
        echo -e "${RED}Error: IP address required${NC}"
        exit 1
    fi
    
    echo -e "${BLUE}Unbanning IP: $ip${NC}"
    
    local data="{\"ip\": \"$ip\"}"
    local response
    response=$(api_call "/p2p/unban" "POST" "$data")
    
    if [[ $? -eq 0 ]]; then
        echo -e "${GREEN}IP unbanned${NC}"
    else
        echo -e "${RED}Failed to unban IP${NC}"
        exit 1
    fi
}

function list_banned() {
    echo -e "${BLUE}Banned IP Addresses${NC}"
    echo "==================="
    
    local response
    response=$(api_call "/p2p/banned")
    
    if [[ $? -eq 0 ]]; then
        local ban_count
        ban_count=$(echo "$response" | jq '. | length')
        
        if [[ "$ban_count" -eq 0 ]]; then
            echo "No banned IPs"
        else
            echo "$response" | jq -r '.[] | "\(.ip) - Banned: \(.banned_at) - Expires: \(.expires_at // "Never")"'
        fi
    else
        echo -e "${RED}Failed to get banned IP list${NC}"
        exit 1
    fi
}

function show_ratelimit() {
    echo -e "${BLUE}Rate Limiting Status${NC}"
    echo "===================="
    
    local response
    response=$(api_call "/p2p/ratelimit")
    
    if [[ $? -eq 0 ]]; then
        format_output "$response"
    else
        echo -e "${RED}Failed to get rate limit status${NC}"
        exit 1
    fi
}

function reset_ratelimit() {
    local peer_id="$1"
    
    if [[ -z "$peer_id" ]]; then
        echo -e "${RED}Error: Peer ID required${NC}"
        exit 1
    fi
    
    echo -e "${BLUE}Resetting rate limit for peer: $peer_id${NC}"
    
    local data="{\"peer_id\": \"$peer_id\"}"
    local response
    response=$(api_call "/p2p/ratelimit/reset" "POST" "$data")
    
    if [[ $? -eq 0 ]]; then
        echo -e "${GREEN}Rate limit reset${NC}"
    else
        echo -e "${RED}Failed to reset rate limit${NC}"
        exit 1
    fi
}

function show_health() {
    echo -e "${BLUE}Network Health Report${NC}"
    echo "====================="
    
    local response
    response=$(api_call "/p2p/health")
    
    if [[ $? -eq 0 ]]; then
        format_output "$response"
    else
        echo -e "${RED}Failed to get health report${NC}"
        exit 1
    fi
}

function toggle_emergency() {
    local action="$1"
    
    if [[ "$action" != "on" && "$action" != "off" ]]; then
        echo -e "${RED}Error: Use 'emergency on' or 'emergency off'${NC}"
        exit 1
    fi
    
    echo -e "${BLUE}Emergency mode: $action${NC}"
    
    local endpoint="/p2p/emergency/$action"
    local response
    response=$(api_call "$endpoint" "POST")
    
    if [[ $? -eq 0 ]]; then
        echo -e "${GREEN}Emergency mode ${action}${NC}"
    else
        echo -e "${RED}Failed to toggle emergency mode${NC}"
        exit 1
    fi
}

function show_metrics() {
    echo -e "${BLUE}Network Metrics${NC}"
    echo "==============="
    
    local response
    response=$(api_call "/p2p/metrics")
    
    if [[ $? -eq 0 ]]; then
        if [[ "$JSON_OUTPUT" == "true" ]]; then
            format_output "$response"
        else
            # Format metrics nicely
            echo "$response" | jq -r 'to_entries[] | "\(.key): \(.value)"'
        fi
    else
        echo -e "${RED}Failed to get metrics${NC}"
        exit 1
    fi
}

function monitor_network() {
    echo -e "${BLUE}Real-time Network Monitor${NC}"
    echo "========================="
    echo "Press Ctrl+C to stop"
    echo ""
    
    while true; do
        clear
        echo -e "${CYAN}MultiVM P2P Network Monitor - $(date)${NC}"
        echo "======================================================"
        
        # Get status
        local status_response
        status_response=$(api_call "/p2p/status" 2>/dev/null)
        
        if [[ $? -eq 0 ]]; then
            local peer_count health_status active_connections
            peer_count=$(echo "$status_response" | jq -r '.peer_count // 0')
            health_status=$(echo "$status_response" | jq -r '.health_status // "Unknown"')
            active_connections=$(echo "$status_response" | jq -r '.active_connections // 0')
            
            echo -e "Peers: ${GREEN}$peer_count${NC} | Connections: ${GREEN}$active_connections${NC} | Health: ${GREEN}$health_status${NC}"
        else
            echo -e "${RED}Unable to connect to P2P API${NC}"
        fi
        
        # Get metrics
        local metrics_response
        metrics_response=$(api_call "/p2p/metrics" 2>/dev/null)
        
        if [[ $? -eq 0 ]]; then
            echo ""
            echo "Key Metrics:"
            echo "$metrics_response" | jq -r 'to_entries[] | select(.key | test("messages_sent|messages_received|network_health_score|cpu_usage")) | "  \(.key): \(.value)"'
        fi
        
        sleep 2
    done
}

function show_alerts() {
    echo -e "${BLUE}Active Alerts${NC}"
    echo "============="
    
    local response
    response=$(api_call "/p2p/alerts")
    
    if [[ $? -eq 0 ]]; then
        local alert_count
        alert_count=$(echo "$response" | jq '. | length')
        
        if [[ "$alert_count" -eq 0 ]]; then
            echo -e "${GREEN}No active alerts${NC}"
        else
            echo "$response" | jq -r '.[] | "\(.severity | ascii_upcase): \(.message) (triggered: \(.triggered_at))"'
        fi
    else
        echo -e "${RED}Failed to get alerts${NC}"
        exit 1
    fi
}

function manage_logs() {
    local level="$1"
    
    if [[ -n "$level" ]]; then
        echo -e "${BLUE}Setting log level to: $level${NC}"
        
        local data="{\"level\": \"$level\"}"
        local response
        response=$(api_call "/p2p/logs/level" "POST" "$data")
        
        if [[ $? -eq 0 ]]; then
            echo -e "${GREEN}Log level set${NC}"
        else
            echo -e "${RED}Failed to set log level${NC}"
            exit 1
        fi
    else
        echo -e "${BLUE}Recent Logs${NC}"
        echo "==========="
        
        local response
        response=$(api_call "/p2p/logs")
        
        if [[ $? -eq 0 ]]; then
            echo "$response" | jq -r '.logs[] | "\(.timestamp) [\(.level)] \(.message)"'
        else
            echo -e "${RED}Failed to get logs${NC}"
            exit 1
        fi
    fi
}

# Parse command line arguments
JSON_OUTPUT=false
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_help
            exit 0
            ;;
        -u|--url)
            API_URL="$2"
            shift 2
            ;;
        -t|--token)
            AUTH_TOKEN="$2"
            shift 2
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -j|--json)
            JSON_OUTPUT=true
            shift
            ;;
        *)
            break
            ;;
    esac
done

# Main command processing
case "$1" in
    status)
        show_status
        ;;
    peers)
        case "$2" in
            list)
                list_peers
                ;;
            info)
                get_peer_info "$3"
                ;;
            connect)
                connect_peer "$3"
                ;;
            disconnect)
                disconnect_peer "$3"
                ;;
            *)
                echo -e "${RED}Invalid peers subcommand. Use: list, info <id>, connect <address>, disconnect <id>${NC}"
                exit 1
                ;;
        esac
        ;;
    ban)
        ban_ip "$2" "$3"
        ;;
    unban)
        unban_ip "$2"
        ;;
    banned)
        list_banned
        ;;
    ratelimit)
        if [[ "$2" == "reset" ]]; then
            reset_ratelimit "$3"
        else
            show_ratelimit
        fi
        ;;
    health)
        show_health
        ;;
    emergency)
        toggle_emergency "$2"
        ;;
    metrics)
        show_metrics
        ;;
    monitor)
        monitor_network
        ;;
    alerts)
        show_alerts
        ;;
    logs)
        manage_logs "$2"
        ;;
    ""|help)
        show_help
        ;;
    *)
        echo -e "${RED}Unknown command: $1${NC}"
        show_help
        exit 1
        ;;
esac