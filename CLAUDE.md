# Claude Code Development Guidelines

**Project Version:** v8.23.25.1  
**Last Updated:** 2025-08-23 - Removed non-working Claude Code hooks and git sync scripts  
**Related Files:** `STATUS.md`, `README.md`

## Project Overview
High-frequency algorithmic trading system with <5ms latency requirements for momentum/breakout trading strategies.

**📋 Documentation Reference:**
- `README_orig.md` - Complete project setup and architecture guide
- `STATUS.md` - **Current implementation status and progress tracking**
- `README.md` - Environment setup, testing endpoints, and user information
- `docs/module_overview.pdf` - Detailed module specifications and requirements
- `docs/DesignQuestions1.pdf` - Performance requirements and trading strategy details

## Code Style Guide & Standards

### File Header Requirements
**ALL source files must begin with file path comment:**
```rust
// src/rules/mod.rs
// src/broker/lightspeed.rs
// src/data/polygon.rs
```

### Documentation Standards
- **All public functions** must have rustdoc comments with examples
- **Complex algorithms** must have inline explanations
- **Performance-critical sections** must document optimization rationale
- **Error handling** must document expected error conditions

### Code Style Requirements
```rust
// Good example with proper documentation
// src/rules/indicators.rs

/// Calculates Exponential Moving Average for given period
/// 
/// # Arguments
/// * `prices` - Vector of price data points
/// * `period` - Number of periods for EMA calculation
/// 
/// # Returns
/// * `Result<f64>` - EMA value or calculation error
/// 
/// # Performance Note
/// Optimized for <1ms execution to meet 5ms total latency requirement
pub fn calculate_ema(prices: &[f64], period: u32) -> Result<f64> {
    // Implementation...
}
```

### Preservation of Existing Functionality
**CRITICAL REQUIREMENTS:**
1. **Always read existing code thoroughly before modifying**
2. **Preserve all existing API contracts and function signatures**
3. **Add functionality, don't replace unless explicitly requested**
4. **Test existing endpoints after any changes**
5. **Maintain backward compatibility with current integrations**

### Error Handling Standards
- Use `anyhow::Result<T>` for error propagation
- Include context with `.context("descriptive message")`
- Log errors with appropriate level (error!, warn!, info!)
- Never panic in production code paths

## Docker-First Development Environment

### System Configuration
- ✅ Docker v28.3.3 available
- ✅ Docker Compose v2.39.1 available  
- ❌ Rust toolchain not installed locally → **Must use Docker**

### Development Commands
```bash
# Start development environment
docker compose up --build

# Code development inside container
docker compose exec trading-system cargo build
docker compose exec trading-system cargo test
docker compose exec trading-system cargo clippy
docker compose exec trading-system cargo fmt

# Quick checks
docker compose run --rm trading-system cargo check

# View logs and debugging
docker compose logs -f trading-system

# Stop services
docker compose down
```

## File Organization Standards

### Module Structure
```
src/
├── main.rs                 // Application entry point
├── config/mod.rs           // ✅ Complete - Environment configuration
├── types/mod.rs            // ✅ Complete - Data structures
├── broker/                 // ⏳ Partial - LightSpeed integration (sandbox)
│   ├── mod.rs
│   └── lightspeed.rs
├── data/                   // ⏳ Partial - Polygon.io (free tier only)
│   ├── mod.rs
│   └── polygon.rs
├── engine/mod.rs           // ⏳ Partial - Framework only, needs rule logic
├── web/mod.rs              // ⏳ Partial - CURL APIs work, web UI missing
├── database/mod.rs         // ❌ Placeholder - Connected but not used
├── rules/mod.rs            // ❌ Empty - NEXT IMPLEMENTATION TARGET
└── metrics/mod.rs          // ❌ Placeholder - Basic init only
```

### Testing Strategy
- **Unit tests** for individual functions
- **Integration tests** for module interactions
- **Performance tests** for latency-critical paths
- **API tests** using CURL commands before web UI development

## Performance Architecture Requirements

### Latency Constraints
- **Total pipeline**: <5ms from data ingestion to trade execution
- **Rule evaluation**: Target <1ms for decision logic
- **Memory management**: Use Arc/RwLock for thread safety
- **Async operations**: Tokio runtime for WebSocket/HTTP

### Current Architecture Status
- ✅ **Rust language**: Appropriate for performance requirements
- ✅ **Async framework**: Tokio implementation complete
- ✅ **Memory patterns**: Arc/RwLock usage established
- ❌ **Decision pipeline**: No implementation yet

## Development Workflow

### Before Making Changes
1. **Read STATUS.md** - Understand current implementation state
2. **Review existing code** - Understand current patterns and constraints
3. **Check dependencies** - Note sandbox/free-tier limitations
4. **Plan backward compatibility** - Ensure existing functionality preserved

### During Development
1. **Follow file path comment standard**
2. **Add comprehensive documentation**
3. **Maintain performance focus** 
4. **Test incrementally**
5. **Use proper error handling patterns**

### After Implementation
1. **Test existing functionality** - Ensure nothing broken
2. **Update STATUS.md** - Mark progress and new limitations
3. **Update version numbers** in all three .md files
4. **Commit with descriptive messages**

## Current Development Context

### Immediate Constraints
- **Polygon.io**: FREE tier only (single endpoint tested)
- **LightSpeed**: Sandbox account (BUY orders only tested)
- **Database**: Connected but not integrated with application
- **Web UI**: Placeholder only (CURL APIs functional)

### Next Implementation Priority
**Rules Engine** (`src/rules/`) - Currently empty placeholder
- Technical indicators (MACD, RSI, EMA, SMA)
- Rules criteria evaluation
- Decision pipeline implementation
- Integration with existing broker/data modules

## Version Management & Git Workflow

### Versioning Scheme
- **Format**: `M.DD.YY.I` (Month.Day.Year.Iteration)
- **Examples**: 
  - `8.21.25.1` = August 21, 2025, iteration 1
  - `8.23.25.1` = August 23, 2025, iteration 1
  - `8.23.25.2` = August 23, 2025, iteration 2

### Git Branch Management
1. **main branch**: Stable, production-ready code
2. **Development branches**: Named with version scheme (e.g., `8.23.25.1`)
3. **Branch lifecycle**:
   - Create feature branch from main: `git checkout -b 8.23.25.1`
   - Develop and commit work to feature branch
   - When ready: merge to main and push
   - **Keep branches**: Do NOT delete branches (preserve for reference)
   - Create new version branch for next development cycle

### Documentation Updates
When updating any of the three core .md files:
1. **Update version number** in header (match current git branch)
2. **Update "Last Updated" date** and brief description
3. **Maintain consistency** across `CLAUDE.md`, `README.md`, `STATUS.md`
4. **Commit all three files together** when making project-wide updates

### Claude Session Management
When the user says "claude commit" or "update docs and commit" at the end of a development session, Claude should:
1. **Review ALL modified files** using `git status` and `git diff`:
   - Check what files have been changed during the session
   - Review each change to determine if it should be committed
   - Ask user for guidance on any ambiguous changes
2. **Update all three core .md files** (CLAUDE.md, README.md, STATUS.md):
   - Update version numbers to match current git branch
   - Update "Last Updated" dates to current date
   - Add session summary to STATUS.md
3. **Create comprehensive git commit** with:
   - Include ALL relevant session changes (not just documentation)
   - Stage documentation updates AND other legitimate changes made during session
   - Descriptive commit message summarizing ALL session work
   - Include Claude Code attribution footer
4. **Push changes to remote repository**:
   - Verify local branch matches remote branch name before pushing
   - Check `git branch --show-current` matches the intended remote branch
   - Push the branch to update the remote repo with session changes
   - Ensure all work is preserved and accessible

### Git Best Practices
- **Commit messages**: Use descriptive messages with emoji prefixes
- **Merge strategy**: Fast-forward merges preferred
- **Branch preservation**: Keep all development branches for historical reference
- **Documentation commits**: Include Claude Code attribution when appropriate

---
**Git Branch:** `8.23.25.1` | **Development Phase:** Rules Engine Implementation + Claude Code Integration