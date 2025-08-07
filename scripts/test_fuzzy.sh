#!/bin/bash

# Fuzzy Storage Test Script for Rspamd Telegram Bot
# This script tests the complete fuzzy storage functionality including training and detection

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
RSPAMD_CONTROLLER="http://127.0.0.1:11334"
RSPAMD_SCAN="http://127.0.0.1:11333"
PASSWORD="superSecret"
FLAG="1"
WEIGHT="10"

# Test data
SPAM_SAMPLES=(
    "Free crypto 100% guaranteed profit! Click here now!"
    "Make money fast with this amazing opportunity!"
    "You've won a prize! Claim it immediately!"
    "Limited time offer - don't miss out!"
    "Earn $1000 per day working from home!"
)

SIMILAR_SPAM_SAMPLES=(
    "Free crypto 100% guaranteed profit! Click here now!"
    "Free crypto 99% guaranteed profit! Click here now!"
    "Free crypto 100% guaranteed profit! Click here immediately!"
    "Free crypto 100% guaranteed profit! Click now!"
)

LEGIT_SAMPLES=(
    "Hello everyone, how are you doing today?"
    "Thanks for the information, that's very helpful."
    "I agree with your point about the project."
    "The weather is nice today, isn't it?"
)

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

# Check if Rspamd is running
check_rspamd_status() {
    log_info "Checking Rspamd status..."
    
    if curl -s "$RSPAMD_CONTROLLER/stat" > /dev/null 2>&1; then
        log_success "Rspamd controller is running"
        return 0
    else
        log_error "Rspamd controller is not accessible"
        return 1
    fi
}

# Test fuzzy training
test_fuzzy_training() {
    log_info "Testing fuzzy training functionality..."
    
    local success_count=0
    local total_count=0
    
    for sample in "${SPAM_SAMPLES[@]}"; do
        total_count=$((total_count + 1))
        
        if curl -s -X POST "$RSPAMD_CONTROLLER/fuzzyadd" \
            -H "Password: $PASSWORD" \
            -H "Flag: $FLAG" \
            -H "Weight: $WEIGHT" \
            -d "$sample" > /dev/null 2>&1; then
            log_success "Trained fuzzy storage with: ${sample:0:50}..."
            success_count=$((success_count + 1))
        else
            log_error "Failed to train fuzzy storage with: ${sample:0:50}..."
        fi
    done
    
    if [ $success_count -eq $total_count ]; then
        log_success "All fuzzy training tests passed ($success_count/$total_count)"
        return 0
    else
        log_error "Some fuzzy training tests failed ($success_count/$total_count)"
        return 1
    fi
}

# Test fuzzy detection
test_fuzzy_detection() {
    log_info "Testing fuzzy detection functionality..."
    
    local success_count=0
    local total_count=0
    
    # Test detection of trained spam
    for sample in "${SPAM_SAMPLES[@]}"; do
        total_count=$((total_count + 1))
        
        local response=$(curl -s -X POST "$RSPAMD_SCAN/checkv2" \
            -H "Content-Type: application/json" \
            -d "{
                \"message\": \"$sample\",
                \"task\": {
                    \"user\": \"test_user\",
                    \"ip\": \"127.0.0.1\"
                }
            }")
        
        if echo "$response" | grep -q "FUZZY_DENIED"; then
            log_success "Detected fuzzy spam: ${sample:0:50}..."
            success_count=$((success_count + 1))
        else
            log_warning "Did not detect fuzzy spam: ${sample:0:50}..."
        fi
    done
    
    # Test detection of similar spam variants
    for sample in "${SIMILAR_SPAM_SAMPLES[@]}"; do
        total_count=$((total_count + 1))
        
        local response=$(curl -s -X POST "$RSPAMD_SCAN/checkv2" \
            -H "Content-Type: application/json" \
            -d "{
                \"message\": \"$sample\",
                \"task\": {
                    \"user\": \"test_user\",
                    \"ip\": \"127.0.0.1\"
                }
            }")
        
        if echo "$response" | grep -q "FUZZY_DENIED"; then
            log_success "Detected similar fuzzy spam: ${sample:0:50}..."
            success_count=$((success_count + 1))
        else
            log_warning "Did not detect similar fuzzy spam: ${sample:0:50}..."
        fi
    done
    
    if [ $success_count -gt 0 ]; then
        log_success "Fuzzy detection working ($success_count/$total_count detections)"
        return 0
    else
        log_error "No fuzzy detections made ($success_count/$total_count)"
        return 1
    fi
}

# Test false positive prevention
test_false_positive_prevention() {
    log_info "Testing false positive prevention..."
    
    local false_positive_count=0
    local total_count=0
    
    for sample in "${LEGIT_SAMPLES[@]}"; do
        total_count=$((total_count + 1))
        
        local response=$(curl -s -X POST "$RSPAMD_SCAN/checkv2" \
            -H "Content-Type: application/json" \
            -d "{
                \"message\": \"$sample\",
                \"task\": {
                    \"user\": \"test_user\",
                    \"ip\": \"127.0.0.1\"
                }
            }")
        
        if echo "$response" | grep -q "FUZZY_DENIED"; then
            log_error "False positive detected: ${sample:0:50}..."
            false_positive_count=$((false_positive_count + 1))
        else
            log_success "No false positive: ${sample:0:50}..."
        fi
    done
    
    if [ $false_positive_count -eq 0 ]; then
        log_success "No false positives detected ($false_positive_count/$total_count)"
        return 0
    else
        log_warning "Some false positives detected ($false_positive_count/$total_count)"
        return 1
    fi
}

# Test fuzzy statistics
test_fuzzy_statistics() {
    log_info "Testing fuzzy statistics..."
    
    local stats_response=$(curl -s "$RSPAMD_CONTROLLER/fuzzystat")
    
    if [ $? -eq 0 ] && [ -n "$stats_response" ]; then
        log_success "Fuzzy statistics retrieved successfully"
        echo "$stats_response" | jq '.' 2>/dev/null || echo "$stats_response"
        return 0
    else
        log_error "Failed to retrieve fuzzy statistics"
        return 1
    fi
}

# Test bot integration (if bot is running)
test_bot_integration() {
    log_info "Testing bot integration..."
    
    # Check if bot is running by looking for the process
    if pgrep -f "rspamd-telegram-bot" > /dev/null; then
        log_success "Bot process is running"
        return 0
    else
        log_warning "Bot process not found - integration test skipped"
        return 0
    fi
}

# Performance test
test_performance() {
    log_info "Testing fuzzy storage performance..."
    
    local start_time=$(date +%s.%N)
    
    # Send multiple training requests
    for i in {1..10}; do
        curl -s -X POST "$RSPAMD_CONTROLLER/fuzzyadd" \
            -H "Password: $PASSWORD" \
            -H "Flag: $FLAG" \
            -H "Weight: $WEIGHT" \
            -d "Performance test message $i" > /dev/null
    done
    
    local end_time=$(date +%s.%N)
    local duration=$(echo "$end_time - $start_time" | bc)
    
    log_success "Performance test completed in ${duration}s"
    
    if (( $(echo "$duration < 5.0" | bc -l) )); then
        log_success "Performance test passed (under 5 seconds)"
        return 0
    else
        log_warning "Performance test slow (${duration}s)"
        return 1
    fi
}

# Main test execution
main() {
    log_info "Starting fuzzy storage test suite..."
    
    local overall_success=true
    
    # Check prerequisites
    if ! command -v curl &> /dev/null; then
        log_error "curl is required but not installed"
        exit 1
    fi
    
    if ! command -v jq &> /dev/null; then
        log_warning "jq not found - JSON output will not be formatted"
    fi
    
    # Run tests
    check_rspamd_status || overall_success=false
    test_fuzzy_training || overall_success=false
    test_fuzzy_detection || overall_success=false
    test_false_positive_prevention || overall_success=false
    test_fuzzy_statistics || overall_success=false
    test_bot_integration || overall_success=false
    test_performance || overall_success=false
    
    # Summary
    echo
    if [ "$overall_success" = true ]; then
        log_success "All fuzzy storage tests completed successfully!"
        echo
        log_info "Fuzzy storage system is working correctly."
        log_info "The bot will now automatically learn from deleted spam messages."
        log_info "Monitor /var/log/rspamd/rspamd.log for FUZZY_DENIED symbols."
    else
        log_error "Some tests failed. Please check the configuration and try again."
        exit 1
    fi
}

# Run main function
main "$@"
