#!/bin/bash

echo "🚀 Setting up Mini API Server project structure..."

# Create main project directory
PROJECT_DIR="mini-api-server"
mkdir -p "$PROJECT_DIR"
cd "$PROJECT_DIR"

# Create source directory structure
mkdir -p src/{server,plugins,session,auth,handlers,utils,cli,adapters}
mkdir -p examples

echo "📁 Created directory structure:"
echo "mini-api-server/"
echo "├── src/"
echo "│   ├── server/"
echo "│   ├── plugins/"
echo "│   ├── session/"
echo "│   ├── auth/"
echo "│   ├── handlers/"
echo "│   ├── utils/"
echo "│   ├── cli/"
echo "│   └── adapters/"
echo "├── examples/"
echo "└── (config files)"

# Create placeholder files that need to be filled
echo "📝 Creating placeholder files (you'll need to copy content from artifacts):"

# Root level files
touch Cargo.toml
touch build.rs
touch README.md
touch mini-api-server.yaml

# Main source files
touch src/lib.rs
touch src/main.rs
touch src/error.rs
touch src/config.rs

# Server module
touch src/server/mod.rs
touch src/server/router.rs

# Plugins module
touch src/plugins/mod.rs
touch src/plugins/manager.rs
touch src/plugins/mock.rs
touch src/plugins/health.rs
touch src/plugins/auth.rs
touch src/plugins/session.rs
touch src/plugins/logging.rs
touch src/plugins/static_files.rs

# Session module
touch src/session/mod.rs
touch src/session/memory.rs
touch src/session/file.rs
touch src/session/sqlite.rs

# Auth module
touch src/auth/mod.rs
touch src/auth/basic.rs
touch src/auth/bearer.rs
touch src/auth/api_key.rs

# Handlers module
touch src/handlers/mod.rs
touch src/handlers/static_files.rs
touch src/handlers/proxy.rs

# Utils module
touch src/utils/mod.rs
touch src/utils/templates.rs
touch src/utils/logging.rs

# CLI module
touch src/cli/mod.rs
touch src/cli/daemon.rs
touch src/cli/commands.rs

# Adapters module
touch src/adapters/mod.rs
touch src/adapters/redis.rs
touch src/adapters/database.rs
touch src/adapters/storage.rs

# Examples
touch examples/complete_example.rs

echo ""
echo "✅ Project structure created!"
echo ""
echo "📋 Next steps:"
echo "1. Copy content from each artifact into the corresponding file"
echo "2. Run 'cargo check' to verify everything compiles"
echo "3. Run 'cargo build --release' to build the server"
echo ""
echo "📁 Files to copy from artifacts:"
echo ""
echo "ROOT LEVEL:"
echo "  Cargo.toml → Cargo.toml"
echo "  build.rs → build.rs" 
echo "  README.md → README.md"
echo "  mini-api-server.yaml → mini-api-server.yaml"
echo ""
echo "MAIN SOURCE:"
echo "  src/lib.rs → src/lib.rs"
echo "  src/main.rs → src/main.rs"
echo "  src/error.rs → src/error.rs"
echo "  src/config.rs → src/config.rs"
echo ""
echo "SERVER MODULE:"
echo "  src/server/mod.rs → src/server/mod.rs"
echo "  src/server/router.rs → src/server/router.rs"
echo ""
echo "PLUGINS MODULE:"
echo "  src/plugins/mod.rs → src/plugins/mod.rs"
echo "  src/plugins/manager.rs → src/plugins/manager.rs"
echo "  src/plugins/mock.rs → src/plugins/mock.rs"
echo "  src/plugins/health.rs → src/plugins/health.rs"
echo ""
echo "SESSION MODULE:"
echo "  src/session/mod.rs → src/session/mod.rs"
echo "  src/session/memory.rs → src/session/memory.rs"
echo "  src/session/file.rs → src/session/file.rs"
echo "  src/session/sqlite.rs → src/session/sqlite.rs"
echo ""
echo "AUTH MODULE:"
echo "  src/auth/mod.rs → src/auth/mod.rs"
echo "  src/auth/basic.rs → src/auth/basic.rs"
echo "  src/auth/bearer.rs → src/auth/bearer.rs"
echo "  src/auth/api_key.rs → src/auth/api_key.rs"
echo ""
echo "HANDLERS MODULE:"
echo "  src/handlers/mod.rs → src/handlers/mod.rs"
echo "  src/handlers/static_files.rs → src/handlers/static_files.rs"
echo "  src/handlers/proxy.rs → src/handlers/proxy.rs"
echo ""
echo "UTILS MODULE:"
echo "  src/utils/mod.rs → src/utils/mod.rs"
echo "  src/utils/templates.rs → src/utils/templates.rs"
echo "  src/utils/logging.rs → src/utils/logging.rs"
echo ""
echo "CLI MODULE:"
echo "  src/cli/mod.rs → src/cli/mod.rs"
echo "  src/cli/daemon.rs → src/cli/daemon.rs"
echo "  src/cli/commands.rs → src/cli/commands.rs"
echo ""
echo "ADAPTERS MODULE:"
echo "  src/adapters/mod.rs → src/adapters/mod.rs"
echo "  src/adapters/redis.rs → src/adapters/redis.rs"
echo "  src/adapters/storage.rs → src/adapters/storage.rs"
echo ""
echo "EXAMPLES:"
echo "  examples/complete_example.rs → examples/complete_example.rs"
echo ""
echo "🎯 Pro tip: Copy files in this order to minimize compilation errors:"
echo "1. Cargo.toml, build.rs (build files)"
echo "2. src/error.rs (base types)"
echo "3. src/config.rs (configuration)"
echo "4. src/lib.rs (module declarations)"
echo "5. All the module files"
echo "6. src/main.rs (entry point)"
echo "7. Examples and config"

cd ..
echo ""
echo "🚀 Ready to build your modular API server!"
