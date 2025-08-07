#!/bin/bash

# Fuzzy Storage Configuration Validation Script
# This script validates that all fuzzy storage configuration files are properly set up

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration paths
RSPAMD_CONFIG_DIR="./rspamd-config/local.d"
FUZZY_WORKER_CONF="$RSPAMD_CONFIG_DIR/worker-fuzzy.conf"
FUZZY_CHECK_CONF="$RSPAMD_CONFIG_DIR/fuzzy_check.conf"
CONTROLLER_CONF="$RSPAMD_CONFIG_DIR/worker-controller.conf"

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

# Check if configuration directory exists
check_config_directory() {
    log_info "Checking Rspamd configuration directory..."
    
    if [ -d "$RSPAMD_CONFIG_DIR" ]; then
        log_success "Configuration directory exists: $RSPAMD_CONFIG_DIR"
        return 0
    else
        log_error "Configuration directory not found: $RSPAMD_CONFIG_DIR"
        return 1
    fi
}

# Validate fuzzy worker configuration
validate_fuzzy_worker_config() {
    log_info "Validating fuzzy worker configuration..."
    
    if [ ! -f "$FUZZY_WORKER_CONF" ]; then
        log_error "Fuzzy worker configuration file not found: $FUZZY_WORKER_CONF"
        return 1
    fi
    
    local errors=0
    
    # Check for required settings
    if ! grep -q "bind_socket.*11335" "$FUZZY_WORKER_CONF"; then
        log_error "Missing or incorrect bind_socket configuration"
        errors=$((errors + 1))
    fi
    
    if ! grep -q "backend.*sqlite" "$FUZZY_WORKER_CONF"; then
        log_error "Missing or incorrect backend configuration"
        errors=$((errors + 1))
    fi
    
    if ! grep -q "hashfile.*fuzzy.db" "$FUZZY_WORKER_CONF"; then
        log_error "Missing or incorrect hashfile configuration"
        errors=$((errors + 1))
    fi
    
    if ! grep -q "expire.*30d" "$FUZZY_WORKER_CONF"; then
        log_error "Missing or incorrect expire configuration"
        errors=$((errors + 1))
    fi
    
    if ! grep -q "allow_update.*127.0.0.1" "$FUZZY_WORKER_CONF"; then
        log_error "Missing or incorrect allow_update configuration"
        errors=$((errors + 1))
    fi
    
    if [ $errors -eq 0 ]; then
        log_success "Fuzzy worker configuration is valid"
        return 0
    else
        log_error "Fuzzy worker configuration has $errors error(s)"
        return 1
    fi
}

# Validate fuzzy check configuration
validate_fuzzy_check_config() {
    log_info "Validating fuzzy check configuration..."
    
    if [ ! -f "$FUZZY_CHECK_CONF" ]; then
        log_error "Fuzzy check configuration file not found: $FUZZY_CHECK_CONF"
        return 1
    fi
    
    local errors=0
    
    # Check for required settings
    if ! grep -q "min_length.*8" "$FUZZY_CHECK_CONF"; then
        log_error "Missing or incorrect min_length configuration"
        errors=$((errors + 1))
    fi
    
    if ! grep -q "timeout.*1s" "$FUZZY_CHECK_CONF"; then
        log_error "Missing or incorrect timeout configuration"
        errors=$((errors + 1))
    fi
    
    if ! grep -q "servers.*127.0.0.1:11335" "$FUZZY_CHECK_CONF"; then
        log_error "Missing or incorrect servers configuration"
        errors=$((errors + 1))
    fi
    
    if ! grep -q "read_only.*no" "$FUZZY_CHECK_CONF"; then
        log_error "Missing or incorrect read_only configuration"
        errors=$((errors + 1))
    fi
    
    if ! grep -q "FUZZY_DENIED.*1.0" "$FUZZY_CHECK_CONF"; then
        log_error "Missing or incorrect FUZZY_DENIED mapping"
        errors=$((errors + 1))
    fi
    
    if ! grep -q "flag.*1" "$FUZZY_CHECK_CONF"; then
        log_error "Missing or incorrect flag configuration"
        errors=$((errors + 1))
    fi
    
    if ! grep -q "max_score.*10" "$FUZZY_CHECK_CONF"; then
        log_error "Missing or incorrect max_score configuration"
        errors=$((errors + 1))
    fi
    
    if [ $errors -eq 0 ]; then
        log_success "Fuzzy check configuration is valid"
        return 0
    else
        log_error "Fuzzy check configuration has $errors error(s)"
        return 1
    fi
}

# Validate controller configuration
validate_controller_config() {
    log_info "Validating controller configuration..."
    
    if [ ! -f "$CONTROLLER_CONF" ]; then
        log_error "Controller configuration file not found: $CONTROLLER_CONF"
        return 1
    fi
    
    local errors=0
    
    # Check for required settings
    if ! grep -q "bind_socket.*11334" "$CONTROLLER_CONF"; then
        log_error "Missing or incorrect bind_socket configuration"
        errors=$((errors + 1))
    fi
    
    if ! grep -q "password.*superSecret" "$CONTROLLER_CONF"; then
        log_error "Missing or incorrect password configuration"
        errors=$((errors + 1))
    fi
    
    if [ $errors -eq 0 ]; then
        log_success "Controller configuration is valid"
        return 0
    else
        log_error "Controller configuration has $errors error(s)"
        return 1
    fi
}

# Check Docker configuration
validate_docker_config() {
    log_info "Validating Docker configuration..."
    
    local errors=0
    
    # Check docker-compose.yml
    if [ -f "docker-compose.yml" ]; then
        if ! grep -q "11335:11335/udp" "docker-compose.yml"; then
            log_error "Missing fuzzy worker port mapping in docker-compose.yml"
            errors=$((errors + 1))
        fi
        
        if ! grep -q "11334:11334" "docker-compose.yml"; then
            log_error "Missing controller port mapping in docker-compose.yml"
            errors=$((errors + 1))
        fi
        
        if ! grep -q "fuzzy-db" "docker-compose.yml"; then
            log_error "Missing fuzzy database volume in docker-compose.yml"
            errors=$((errors + 1))
        fi
    else
        log_warning "docker-compose.yml not found"
    fi
    
    # Check Dockerfile
    if [ -f "docker/Dockerfile" ]; then
        if ! grep -q "EXPOSE.*11335" "docker/Dockerfile"; then
            log_warning "Missing fuzzy worker port exposure in Dockerfile"
        fi
    else
        log_warning "Dockerfile not found"
    fi
    
    if [ $errors -eq 0 ]; then
        log_success "Docker configuration is valid"
        return 0
    else
        log_error "Docker configuration has $errors error(s)"
        return 1
    fi
}

# Check Rust configuration
validate_rust_config() {
    log_info "Validating Rust configuration..."
    
    local errors=0
    
    # Check Cargo.toml for reqwest dependency
    if [ -f "Cargo.toml" ]; then
        if ! grep -q "reqwest.*0.12" "Cargo.toml"; then
            log_error "Missing reqwest dependency in Cargo.toml"
            errors=$((errors + 1))
        fi
    else
        log_error "Cargo.toml not found"
        errors=$((errors + 1))
    fi
    
    # Check for fuzzy trainer module
    if [ ! -f "src/fuzzy_trainer.rs" ]; then
        log_error "Fuzzy trainer module not found: src/fuzzy_trainer.rs"
        errors=$((errors + 1))
    fi
    
    # Check for fuzzy configuration in config.rs
    if [ -f "src/config.rs" ]; then
        if ! grep -q "FUZZY_DENIED" "src/config.rs"; then
            log_error "Missing FUZZY_DENIED symbol in config.rs"
            errors=$((errors + 1))
        fi
        
        if ! grep -q "CONTROLLER_URL" "src/config.rs"; then
            log_error "Missing CONTROLLER_URL in config.rs"
            errors=$((errors + 1))
        fi
    else
        log_error "config.rs not found"
        errors=$((errors + 1))
    fi
    
    if [ $errors -eq 0 ]; then
        log_success "Rust configuration is valid"
        return 0
    else
        log_error "Rust configuration has $errors error(s)"
        return 1
    fi
}

# Check file permissions
check_file_permissions() {
    log_info "Checking file permissions..."
    
    local errors=0
    
    # Check if test script is executable
    if [ -f "scripts/test_fuzzy.sh" ]; then
        if [ ! -x "scripts/test_fuzzy.sh" ]; then
            log_warning "test_fuzzy.sh is not executable"
            chmod +x "scripts/test_fuzzy.sh"
            log_success "Made test_fuzzy.sh executable"
        fi
    fi
    
    if [ -f "scripts/validate_fuzzy_config.sh" ]; then
        if [ ! -x "scripts/validate_fuzzy_config.sh" ]; then
            log_warning "validate_fuzzy_config.sh is not executable"
            chmod +x "scripts/validate_fuzzy_config.sh"
            log_success "Made validate_fuzzy_config.sh executable"
        fi
    fi
    
    return 0
}

# Main validation function
main() {
    log_info "Starting fuzzy storage configuration validation..."
    
    local overall_success=true
    
    check_config_directory || overall_success=false
    validate_fuzzy_worker_config || overall_success=false
    validate_fuzzy_check_config || overall_success=false
    validate_controller_config || overall_success=false
    validate_docker_config || overall_success=false
    validate_rust_config || overall_success=false
    check_file_permissions || overall_success=false
    
    # Summary
    echo
    if [ "$overall_success" = true ]; then
        log_success "All configuration validation tests passed!"
        echo
        log_info "Fuzzy storage configuration is properly set up."
        log_info "You can now run the fuzzy storage tests with: ./scripts/test_fuzzy.sh"
    else
        log_error "Some configuration validation tests failed."
        log_error "Please fix the issues above before running the fuzzy storage tests."
        exit 1
    fi
}

# Run main function
main "$@"
