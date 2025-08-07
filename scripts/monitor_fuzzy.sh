#!/bin/bash

# Fuzzy Storage Monitoring Script
# This script provides real-time monitoring of fuzzy storage performance and statistics

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
RSPAMD_CONTROLLER="http://127.0.0.1:11334"
RSPAMD_SCAN="http://127.0.0.1:11333"
PASSWORD="superSecret"
LOG_FILE="/var/log/rspamd/rspamd.log"
MONITOR_INTERVAL=30

# Helper functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_stats() {
    echo -e "${CYAN}[STATS]${NC} $1"
}

# Get fuzzy statistics
get_fuzzy_stats() {
    local stats_response=$(curl -s "$RSPAMD_CONTROLLER/fuzzystat")
    
    if [ $? -eq 0 ] && [ -n "$stats_response" ]; then
        echo "$stats_response"
    else
        echo "{}"
    fi
}

# Parse fuzzy statistics
parse_fuzzy_stats() {
    local stats_json="$1"
    
    # Extract key metrics using jq if available
    if command -v jq &> /dev/null; then
        local total_hashes=$(echo "$stats_json" | jq -r '.total_hashes // 0')
        local total_checked=$(echo "$stats_json" | jq -r '.total_checked // 0')
        local total_found=$(echo "$stats_json" | jq -r '.total_found // 0')
        local total_learned=$(echo "$stats_json" | jq -r '.total_learned // 0')
        
        echo "$total_hashes|$total_checked|$total_found|$total_learned"
    else
        # Fallback parsing without jq
        local total_hashes=$(echo "$stats_json" | grep -o '"total_hashes":[0-9]*' | cut -d: -f2 || echo "0")
        local total_checked=$(echo "$stats_json" | grep -o '"total_checked":[0-9]*' | cut -d: -f2 || echo "0")
        local total_found=$(echo "$stats_json" | grep -o '"total_found":[0-9]*' | cut -d: -f2 || echo "0")
        local total_learned=$(echo "$stats_json" | grep -o '"total_learned":[0-9]*' | cut -d: -f2 || echo "0")
        
        echo "$total_hashes|$total_checked|$total_found|$total_learned"
    fi
}

# Monitor fuzzy storage activity
monitor_fuzzy_activity() {
    log_info "Starting fuzzy storage monitoring..."
    log_info "Press Ctrl+C to stop monitoring"
    echo
    
    local last_stats=""
    local iteration=0
    
    while true; do
        iteration=$((iteration + 1))
        
        # Get current timestamp
        local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
        
        # Get fuzzy statistics
        local stats_json=$(get_fuzzy_stats)
        local parsed_stats=$(parse_fuzzy_stats "$stats_json")
        
        # Parse statistics
        IFS='|' read -r total_hashes total_checked total_found total_learned <<< "$parsed_stats"
        
        # Calculate rates if we have previous stats
        local hash_rate=""
        local check_rate=""
        local found_rate=""
        local learn_rate=""
        
        if [ -n "$last_stats" ]; then
            IFS='|' read -r last_hashes last_checked last_found last_learned <<< "$last_stats"
            
            local hash_diff=$((total_hashes - last_hashes))
            local check_diff=$((total_checked - last_checked))
            local found_diff=$((total_found - last_found))
            local learn_diff=$((total_learned - last_learned))
            
            hash_rate=" (+${hash_diff})"
            check_rate=" (+${check_diff})"
            found_rate=" (+${found_diff})"
            learn_rate=" (+${learn_diff})"
        fi
        
        # Display statistics
        echo "=== Fuzzy Storage Statistics [$timestamp] ==="
        log_stats "Total Hashes: $total_hashes$hash_rate"
        log_stats "Total Checked: $total_checked$check_rate"
        log_stats "Total Found: $total_found$found_rate"
        log_stats "Total Learned: $total_learned$learn_rate"
        
        # Calculate hit rate
        if [ "$total_checked" -gt 0 ]; then
            local hit_rate=$(echo "scale=2; $total_found * 100 / $total_checked" | bc -l 2>/dev/null || echo "0")
            log_stats "Hit Rate: ${hit_rate}%"
        fi
        
        # Check for recent fuzzy detections in logs
        local recent_detections=$(tail -n 100 "$LOG_FILE" 2>/dev/null | grep -c "FUZZY_DENIED" || echo "0")
        if [ "$recent_detections" -gt 0 ]; then
            log_success "Recent fuzzy detections: $recent_detections"
        fi
        
        # Check for recent fuzzy training in logs
        local recent_training=$(tail -n 100 "$LOG_FILE" 2>/dev/null | grep -c "fuzzyadd" || echo "0")
        if [ "$recent_training" -gt 0 ]; then
            log_success "Recent fuzzy training: $recent_training"
        fi
        
        # Performance indicators
        echo
        echo "=== Performance Indicators ==="
        
        # Check Rspamd response time
        local start_time=$(date +%s.%N)
        curl -s "$RSPAMD_CONTROLLER/stat" > /dev/null 2>&1
        local end_time=$(date +%s.%N)
        local response_time=$(echo "$end_time - $start_time" | bc -l 2>/dev/null || echo "0")
        
        if (( $(echo "$response_time < 0.1" | bc -l) )); then
            log_success "Response Time: ${response_time}s (Excellent)"
        elif (( $(echo "$response_time < 0.5" | bc -l) )); then
            log_success "Response Time: ${response_time}s (Good)"
        elif (( $(echo "$response_time < 1.0" | bc -l) )); then
            log_warning "Response Time: ${response_time}s (Acceptable)"
        else
            log_error "Response Time: ${response_time}s (Slow)"
        fi
        
        # Check memory usage
        local memory_usage=$(ps aux | grep rspamd | grep -v grep | awk '{sum+=$6} END {print sum/1024}' 2>/dev/null || echo "0")
        if (( $(echo "$memory_usage < 100" | bc -l) )); then
            log_success "Memory Usage: ${memory_usage}MB (Low)"
        elif (( $(echo "$memory_usage < 500" | bc -l) )); then
            log_success "Memory Usage: ${memory_usage}MB (Normal)"
        else
            log_warning "Memory Usage: ${memory_usage}MB (High)"
        fi
        
        # Check disk usage for fuzzy database
        local disk_usage=$(du -sh /var/lib/rspamd/fuzzy.db 2>/dev/null | cut -f1 || echo "0")
        log_stats "Fuzzy DB Size: $disk_usage"
        
        # Store current stats for next iteration
        last_stats="$total_hashes|$total_checked|$total_found|$total_learned"
        
        echo
        echo "=== Recent Log Activity ==="
        
        # Show recent fuzzy-related log entries
        local recent_logs=$(tail -n 20 "$LOG_FILE" 2>/dev/null | grep -i "fuzzy\|FUZZY_DENIED" | tail -n 5 || echo "No recent fuzzy activity")
        if [ "$recent_logs" != "No recent fuzzy activity" ]; then
            echo "$recent_logs" | while IFS= read -r line; do
                log_info "$line"
            done
        else
            log_info "No recent fuzzy activity detected"
        fi
        
        echo
        echo "Monitoring will update in ${MONITOR_INTERVAL} seconds... (Iteration $iteration)"
        echo "Press Ctrl+C to stop"
        echo
        
        # Wait for next iteration
        sleep $MONITOR_INTERVAL
    done
}

# Show fuzzy storage configuration
show_fuzzy_config() {
    log_info "Fuzzy Storage Configuration:"
    echo
    
    # Check fuzzy worker config
    if [ -f "rspamd-config/local.d/worker-fuzzy.conf" ]; then
        log_success "Fuzzy Worker Config:"
        cat "rspamd-config/local.d/worker-fuzzy.conf" | sed 's/^/  /'
        echo
    else
        log_error "Fuzzy worker config not found"
    fi
    
    # Check fuzzy check config
    if [ -f "rspamd-config/local.d/fuzzy_check.conf" ]; then
        log_success "Fuzzy Check Config:"
        cat "rspamd-config/local.d/fuzzy_check.conf" | sed 's/^/  /'
        echo
    else
        log_error "Fuzzy check config not found"
    fi
    
    # Check controller config
    if [ -f "rspamd-config/local.d/worker-controller.conf" ]; then
        log_success "Controller Config:"
        cat "rspamd-config/local.d/worker-controller.conf" | sed 's/^/  /'
        echo
    else
        log_error "Controller config not found"
    fi
}

# Show help
show_help() {
    echo "Fuzzy Storage Monitor"
    echo
    echo "Usage: $0 [OPTION]"
    echo
    echo "Options:"
    echo "  monitor    Start real-time monitoring (default)"
    echo "  config     Show fuzzy storage configuration"
    echo "  stats      Show current fuzzy statistics"
    echo "  help       Show this help message"
    echo
    echo "Examples:"
    echo "  $0 monitor    # Start monitoring"
    echo "  $0 config     # Show configuration"
    echo "  $0 stats      # Show current stats"
}

# Show current statistics
show_current_stats() {
    log_info "Current Fuzzy Storage Statistics:"
    echo
    
    local stats_json=$(get_fuzzy_stats)
    
    if command -v jq &> /dev/null; then
        echo "$stats_json" | jq '.'
    else
        echo "$stats_json"
    fi
}

# Main function
main() {
    case "${1:-monitor}" in
        "monitor")
            monitor_fuzzy_activity
            ;;
        "config")
            show_fuzzy_config
            ;;
        "stats")
            show_current_stats
            ;;
        "help"|"-h"|"--help")
            show_help
            ;;
        *)
            log_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
}

# Check prerequisites
if ! command -v curl &> /dev/null; then
    log_error "curl is required but not installed"
    exit 1
fi

if ! command -v bc &> /dev/null; then
    log_warning "bc not found - some calculations may not work"
fi

# Run main function
main "$@"
