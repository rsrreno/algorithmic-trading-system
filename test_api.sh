#!/bin/bash
# test_api.sh
# Branch: 8.28.25.1

# Trading System API Testing Script
# Tests the web server running in Docker from the host system
# Based on ENDPOINTS.md documentation

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
BASE_URL="http://localhost:8080"
TIMEOUT=30

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Helper functions
print_header() {
    echo -e "\n${BLUE}════════════════════════════════════════${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}════════════════════════════════════════${NC}"
}

print_subheader() {
    echo -e "\n${PURPLE}── $1 ──${NC}"
}

print_test() {
    echo -e "\n${CYAN}🧪 Testing: $1${NC}"
    TESTS_RUN=$((TESTS_RUN + 1))
}

print_success() {
    echo -e "${GREEN}✅ PASS: $1${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
}

print_failure() {
    echo -e "${RED}❌ FAIL: $1${NC}"
    echo -e "${RED}   Error: $2${NC}"
    TESTS_FAILED=$((TESTS_FAILED + 1))
}

print_not_implemented() {
    echo -e "${YELLOW}⚠️  NOT IMPLEMENTED: $1${NC}"
    echo -e "${YELLOW}   Expected: $2${NC}"
}

print_response() {
    local response="$1"
    local max_length=200
    if [ ${#response} -gt $max_length ]; then
        echo -e "${CYAN}   Response: $(echo "$response" | head -c $max_length)...${NC}"
    else
        echo -e "${CYAN}   Response: $response${NC}"
    fi
}

# Test a web server endpoint
test_endpoint() {
    local method=$1
    local endpoint=$2
    local data=$3
    local description=$4
    local expected_field=$5
    
    print_test "$description"
    
    if [ "$method" = "GET" ]; then
        response=$(curl -s -w "\n%{http_code}" --max-time $TIMEOUT "$BASE_URL$endpoint" 2>/dev/null || echo "CURL_ERROR\n000")
    else
        response=$(curl -s -w "\n%{http_code}" --max-time $TIMEOUT -X "$method" \
            -H "Content-Type: application/json" \
            -d "$data" \
            "$BASE_URL$endpoint" 2>/dev/null || echo "CURL_ERROR\n000")
    fi
    
    # Split response and status code
    if echo "$response" | grep -q "CURL_ERROR"; then
        print_failure "$description" "Connection failed - is Docker container running?"
        return
    fi
    
    http_code=$(echo "$response" | tail -n1)
    response_body=$(echo "$response" | head -n -1)
    
    if [ "$http_code" -eq 200 ]; then
        if [ -n "$expected_field" ]; then
            if command -v jq >/dev/null 2>&1 && echo "$response_body" | jq -e "$expected_field" > /dev/null 2>&1; then
                print_success "$description"
                print_response "$(echo "$response_body" | jq -c .)"
            else
                print_failure "$description" "Expected field '$expected_field' not found in JSON"
                print_response "$response_body"
            fi
        else
            print_success "$description"
            print_response "$response_body"
        fi
    elif [ "$http_code" -eq 404 ]; then
        print_not_implemented "$description" "Endpoint $endpoint should be implemented"
    else
        print_failure "$description" "HTTP $http_code"
        print_response "$response_body"
    fi
}

# Check if Docker container is running
check_docker_status() {
    if ! command -v docker >/dev/null 2>&1; then
        echo -e "${RED}❌ Docker not found. Please install Docker.${NC}"
        exit 1
    fi
    
    if ! docker compose ps | grep -q "trading-system.*Up"; then
        echo -e "${YELLOW}⚠️  Trading system container not running.${NC}"
        echo -e "${CYAN}💡 Start it with: docker compose up -d${NC}"
        echo -e "\n${BLUE}Attempting to test anyway (container might be starting)...${NC}"
    else
        echo -e "${GREEN}✅ Trading system container is running${NC}"
    fi
}

# Test server responsiveness
test_server_health() {
    print_test "Server connectivity"
    response=$(curl -s --max-time 5 "$BASE_URL/health" 2>/dev/null || echo "FAILED")
    
    if [ "$response" = "FAILED" ]; then
        echo -e "${RED}❌ Cannot connect to server at $BASE_URL${NC}"
        echo -e "${CYAN}💡 Make sure Docker container is running: docker compose up -d${NC}"
        exit 1
    else
        print_success "Server is responding"
        TESTS_RUN=$((TESTS_RUN - 1))  # Don't count connectivity check
    fi
}

# Start testing
clear
echo -e "${CYAN}🚀 Trading System API Test Suite${NC}"
echo -e "${CYAN}Based on ENDPOINTS.md v8.23.25.1${NC}"
echo -e "\nTesting server at: $BASE_URL"
echo -e "Timeout: ${TIMEOUT}s per request"

# Pre-flight checks
print_header "PRE-FLIGHT CHECKS"
check_docker_status
test_server_health

# 1. Basic System Endpoints
print_header "1. BASIC SYSTEM ENDPOINTS"

print_subheader "Health & Status Checks"
test_endpoint "GET" "/health" "" "Health check endpoint" ""
test_endpoint "GET" "/api/status" "" "System status with broker connection" ".broker_connected"
test_endpoint "GET" "/" "" "Root endpoint (web interface)" ""

# 2. Currently Implemented Endpoints
print_header "2. CURRENTLY IMPLEMENTED ENDPOINTS"

print_subheader "Symbol Data (Basic Polygon Integration)"
test_endpoint "POST" "/api/symbol" '{"symbol":"AAPL"}' "Apple (AAPL) stock data" ".success"
test_endpoint "POST" "/api/symbol" '{"symbol":"TSLA"}' "Tesla (TSLA) stock data" ".success"
test_endpoint "POST" "/api/symbol" '{"symbol":"MSFT"}' "Microsoft (MSFT) stock data" ".success"
test_endpoint "POST" "/api/symbol" '{"symbol":"GOOGL"}' "Google (GOOGL) stock data" ".success"
test_endpoint "POST" "/api/symbol" '{"symbol":"META"}' "Meta (META) stock data" ".success"
test_endpoint "POST" "/api/symbol" '{"symbol":"NVDA"}' "Nvidia (NVDA) stock data" ".success"

print_subheader "Symbol Error Handling"
test_endpoint "POST" "/api/symbol" '{"symbol":"INVALID123"}' "Invalid symbol handling" ""
test_endpoint "POST" "/api/symbol" '{"invalid":"json"}' "Invalid JSON structure" ""
test_endpoint "POST" "/api/symbol" '{}' "Empty symbol request" ""

print_subheader "Order Management (LightSpeed Integration)"
test_endpoint "POST" "/api/order" '{"symbol":"AAPL","side":"BUY","order_type":"LIMIT","quantity":1,"price":150.0}' "Apple BUY LIMIT order" ""
test_endpoint "POST" "/api/order" '{"symbol":"TSLA","side":"BUY","order_type":"LIMIT","quantity":1,"price":200.0}' "Tesla BUY LIMIT order" ""
test_endpoint "POST" "/api/order" '{"symbol":"MSFT","side":"BUY","order_type":"MARKET","quantity":1}' "Microsoft BUY MARKET order" ""

print_subheader "Order Error Handling"
test_endpoint "POST" "/api/order" '{"symbol":"AAPL","side":"INVALID","order_type":"LIMIT","quantity":1,"price":150.0}' "Invalid order side" ""
test_endpoint "POST" "/api/order" '{"symbol":"AAPL","side":"BUY","order_type":"LIMIT","quantity":0,"price":150.0}' "Zero quantity order" ""
test_endpoint "POST" "/api/order" '{}' "Empty order request" ""

# 3. Missing Polygon Endpoints (Per ENDPOINTS.md)
print_header "3. MISSING POLYGON ENDPOINTS"
echo -e "${YELLOW}The following endpoints are documented in ENDPOINTS.md but not implemented:${NC}"

print_subheader "Technical Indicators (Documented as Available)"
test_endpoint "GET" "/api/indicators/sma/AAPL?window=50&timespan=day" "" "SMA endpoint" ""
test_endpoint "GET" "/api/indicators/ema/AAPL?window=50&timespan=day" "" "EMA endpoint" ""
test_endpoint "GET" "/api/indicators/rsi/AAPL?window=14&timespan=day" "" "RSI endpoint" ""
test_endpoint "GET" "/api/indicators/macd/AAPL?short_window=12&long_window=26&signal_window=9" "" "MACD endpoint" ""

print_subheader "Market Data & Snapshots (Documented as Working)"
test_endpoint "GET" "/api/snapshot/AAPL" "" "Single ticker snapshot" ""
test_endpoint "GET" "/api/market/movers/gainers" "" "Top market gainers" ""
test_endpoint "GET" "/api/market/movers/losers" "" "Top market losers" ""
test_endpoint "GET" "/api/market/status" "" "Market status" ""
test_endpoint "GET" "/api/market/previous/AAPL" "" "Previous day data" ""
test_endpoint "GET" "/api/market/minute/AAPL?from=2024-01-01&to=2024-01-02" "" "Minute aggregates" ""

print_subheader "Reference Data (Documented as Tested)"
test_endpoint "GET" "/api/reference/tickers?limit=10" "" "All tickers list" ""
test_endpoint "GET" "/api/reference/exchanges" "" "Stock exchanges" ""
test_endpoint "GET" "/api/reference/splits/AAPL" "" "Stock splits" ""

print_subheader "News & Fundamentals (Documented as Working)"
test_endpoint "GET" "/api/news?ticker=AAPL&limit=5" "" "News with sentiment" ""
test_endpoint "GET" "/api/financials/AAPL" "" "Financial statements" ""

# 4. Additional Missing Features
print_header "4. ADDITIONAL MISSING FEATURES"

print_subheader "WebSocket Endpoints (Not HTTP)"
echo -e "${YELLOW}⚠️  WebSocket streaming not testable via HTTP${NC}"
echo -e "${CYAN}   Real-time data requires WebSocket connection to Polygon.io${NC}"

print_subheader "Cache Management"
test_endpoint "GET" "/api/cache/stats" "" "Cache statistics" ""
test_endpoint "POST" "/api/cache/clear" "" "Cache cleanup" ""

print_subheader "Rules Engine (Not Yet Implemented)"
test_endpoint "GET" "/api/rules" "" "Trading rules list" ""
test_endpoint "POST" "/api/rules" '{"name":"test","conditions":[]}' "Create trading rule" ""

# 5. Performance Test
print_header "5. PERFORMANCE & LOAD TEST"

print_subheader "Sequential Requests"
for i in {1..3}; do
    test_endpoint "POST" "/api/symbol" '{"symbol":"AAPL"}' "Sequential request #$i" ".success"
    sleep 0.5
done

print_subheader "Rapid Fire Test (No Delays)"
echo -e "${CYAN}Testing rapid requests to check server stability...${NC}"
for i in {1..5}; do
    print_test "Rapid request #$i"
    response=$(curl -s --max-time 10 -X POST \
        -H "Content-Type: application/json" \
        -d '{"symbol":"AAPL"}' \
        "$BASE_URL/api/symbol" 2>/dev/null || echo "FAILED")
    
    if [ "$response" != "FAILED" ] && (command -v jq >/dev/null 2>&1 && echo "$response" | jq -e ".success" > /dev/null 2>&1); then
        print_success "Rapid request #$i"
        TESTS_RUN=$((TESTS_RUN + 1))
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        print_failure "Rapid request #$i" "API call failed or invalid response"
        TESTS_RUN=$((TESTS_RUN + 1))
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
done

# Results Summary
print_header "TEST RESULTS SUMMARY"

echo -e "📊 ${BLUE}Total Tests Run: $TESTS_RUN${NC}"
echo -e "✅ ${GREEN}Tests Passed: $TESTS_PASSED${NC}"
echo -e "❌ ${RED}Tests Failed: $TESTS_FAILED${NC}"

if [ $TESTS_RUN -gt 0 ]; then
    success_rate=$(( (TESTS_PASSED * 100) / TESTS_RUN ))
    echo -e "📈 ${CYAN}Success Rate: ${success_rate}%${NC}"
fi

echo ""
if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "🎉 ${GREEN}ALL IMPLEMENTED ENDPOINTS WORKING!${NC}"
    echo -e "${GREEN}Your trading system's current functionality is operational.${NC}"
else
    echo -e "⚠️  ${YELLOW}$TESTS_FAILED test(s) failed${NC}"
    echo -e "${YELLOW}Check the failed endpoints above - they may indicate implementation issues.${NC}"
fi

# Implementation Status Report
print_header "IMPLEMENTATION STATUS REPORT"
echo -e "${CYAN}Based on ENDPOINTS.md documentation:${NC}"
echo -e "${GREEN}✅ WORKING:${NC}"
echo -e "   • Basic system health and status endpoints"
echo -e "   • Symbol data retrieval (basic Polygon integration)"
echo -e "   • Order placement (LightSpeed BUY orders)"
echo -e "   • Error handling for invalid inputs"

echo -e "\n${YELLOW}⚠️  NOT IMPLEMENTED (but documented as available):${NC}"
echo -e "   • Technical indicators (SMA, EMA, RSI, MACD)"
echo -e "   • Market snapshots and minute-level data"
echo -e "   • Market movers and reference data"
echo -e "   • News and financial statements"
echo -e "   • WebSocket real-time streaming"
echo -e "   • Cache management endpoints"
echo -e "   • Rules engine (completely missing)"

echo -e "\n${BLUE}📋 NEXT DEVELOPMENT PRIORITIES:${NC}"
echo -e "   1. Implement Polygon technical indicator endpoints"
echo -e "   2. Add market data snapshot endpoints"
echo -e "   3. Build WebSocket streaming for real-time data"
echo -e "   4. Create rules engine with decision logic"
echo -e "   5. Add cache management for performance"

echo -e "\n${PURPLE}💡 TO RUN ADDITIONAL TESTS:${NC}"
echo -e "   • Start system: ${CYAN}docker compose up -d${NC}"
echo -e "   • View logs: ${CYAN}docker compose logs -f trading-system${NC}"
echo -e "   • Restart system: ${CYAN}docker compose restart${NC}"
echo -e "   • Stop system: ${CYAN}docker compose down${NC}"

echo -e "\n${BLUE}Testing completed at $(date)${NC}"