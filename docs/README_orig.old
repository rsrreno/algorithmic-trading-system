# Trading System - Project Setup

## Complete Project Structure

```
trading-system/
├── .github/
│   └── workflows/
│       └── ci.yml                  # GitHub Actions CI/CD
├── src/
│   ├── main.rs                     # Application entry point
│   ├── config/
│   │   └── mod.rs                  # Configuration management
│   ├── engine/
│   │   └── mod.rs                  # Core trading engine
│   ├── data/
│   │   ├── mod.rs                  # Data module interface
│   │   ├── polygon.rs              # Polygon.io integration
│   │   └── stream.rs               # Market data streaming
│   ├── broker/
│   │   ├── mod.rs                  # Broker module interface
│   │   └── lightspeed.rs           # LightSpeed Connect integration
│   ├── rules/
│   │   ├── mod.rs                  # Rules engine
│   │   ├── engine.rs               # Rule evaluation logic
│   │   └── indicators.rs           # Technical indicators
│   ├── web/
│   │   ├── mod.rs                  # Web server
│   │   ├── handlers.rs             # API handlers
│   │   └── static/                 # Web UI files
│   ├── database/
│   │   └── mod.rs                  # Database operations
│   ├── types/
│   │   └── mod.rs                  # Shared types and structures
│   └── metrics/
│       └── mod.rs                  # Performance metrics
├── migrations/
│   ├── 001_initial.sql             # Database schema
│   ├── 002_decisions.sql           # Decision logging tables
│   └── 003_rules.sql               # Rules storage tables
├── deploy/
│   ├── install.sh                  # Production install script
│   ├── systemd/
│   │   └── trading-system.service  # SystemD service file
│   └── config/
│       └── production.env          # Production environment template
├── monitoring/
│   └── prometheus.yml              # Prometheus configuration
├── data/                           # Development database (created by Docker)
├── logs/                           # Log files (created by Docker)
├── config/                         # Configuration files
├── Cargo.toml                      # Rust dependencies
├── Cargo.lock                      # Dependency lock file
├── Dockerfile                      # Docker image definition
├── docker-compose.yml              # Development environment
├── .env.example                    # Environment variables template
├── .gitignore                      # Git ignore rules
└── README.md                       # Project documentation
```

## Quick Start Instructions

### 1. Clone and Setup
```bash
# Create project directory
mkdir trading-system
cd trading-system

# Initialize git repository
git init
git remote add origin <your-github-repo-url>

# Create directory structure
mkdir -p src/{config,engine,data,broker,rules,web,database,types,metrics}
mkdir -p migrations deploy/{systemd,config} monitoring data logs config
mkdir -p .github/workflows
```

### 2. Environment Setup
```bash
# Copy the provided files to their locations
# Create .env file for development
cp .env.example .env

# Edit .env with your API keys:
POLYGON_API_KEY=your_polygon_api_key_here
LIGHTSPEED_API_KEY=your_lightspeed_api_key_here
LIGHTSPEED_API_SECRET=your_lightspeed_secret_here
LIGHTSPEED_SANDBOX=true
```

### 3. Development Workflow
```bash
# Start development environment
docker-compose up --build

# For debugging with SQLite browser
docker-compose --profile debug up

# For monitoring with Prometheus
docker-compose --profile monitoring up

# Run tests
docker exec -it trading-system cargo test

# View logs
docker-compose logs -f trading-system
```

### 4. GitHub Repository Setup

#### Required Secrets (Repository Settings → Secrets):
- `POLYGON_API_KEY`: Your Polygon.io API key
- `LIGHTSPEED_API_KEY`: Your LightSpeed API key  
- `LIGHTSPEED_API_SECRET`: Your LightSpeed API secret
- `DOCKER_USERNAME`: Docker Hub username (optional)
- `DOCKER_PASSWORD`: Docker Hub password (optional)

### 5. Production Deployment
```bash
# On your Ubuntu server:
wget <github-release-url>/trading-system-<commit-sha>.tar.gz
tar -xzf trading-system-<commit-sha>.tar.gz
cd deploy-package
sudo ./deploy/install.sh

# Configure environment
sudo nano /opt/trading-system/.env

# Start service
sudo systemctl start trading-system
sudo systemctl status trading-system
```

## Next Development Steps

### Phase 1: Core Infrastructure (Current)
- ✅ Project structure created
- ✅ Build system configured  
- ✅ CI/CD pipeline setup
- ⏳ Database schema design
- ⏳ Basic configuration loading

### Phase 2: Database & Types (Next)
- Define core data types (Position, Trade, MarketData)
- Create database schema and migrations
- Implement memory management for market streams
- Basic logging infrastructure

### Phase 3: Rules Engine
- Technical indicator calculations
- Rule definition and storage
- Web UI for rule management
- Rule evaluation engine

### Phase 4: Data Integration
- Polygon.io REST API client
- WebSocket streaming implementation
- Market data parsing and normalization
- Memory-efficient data structures

### Phase 5: Trading Engine
- Position management
- Decision making logic
- Risk management
- Performance monitoring

### Phase 6: Broker Integration
- LightSpeed Connect API
- Order execution
- Position monitoring
- Stop-loss management

## Monitoring & Debugging

### Application Health
- Web UI: `http://localhost:8080`
- Metrics: `http://localhost:9090/metrics`
- Health Check: `http://localhost:8080/health`

### Database Debugging
- SQLite Browser: `http://localhost:3000` (with debug profile)
- Direct SQLite: `sqlite3 ./data/trading.db`

### Performance Monitoring
- Memory usage tracking built-in
- Prometheus metrics export
- Decision latency measurement
- Trade execution timing

Ready to begin Phase 2 with database design and core types?