#!/bin/bash

echo "🚀 Setting up Nox Server project structure..."

# Work in current directory (should be nox_server)
echo "📁 Working in current directory: $(pwd)"
echo "📁 Checking for src_ref directory..."

if [ ! -d "src_ref" ]; then
    echo "❌ Error: src_ref directory not found! Run this script from the nox_server root directory."
    exit 1
fi

# Create source directory structure
mkdir -p src/{server,plugins,session,auth,handlers,utils,cli,adapters,templates,config,error}
mkdir -p examples

echo "📁 Created directory structure:"
echo "nox_server/"
echo "├── src/"
echo "│   ├── server/"
echo "│   ├── plugins/"
echo "│   ├── session/"
echo "│   ├── auth/"
echo "│   ├── handlers/"
echo "│   ├── utils/"
echo "│   ├── cli/"
echo "│   ├── adapters/"
echo "│   ├── templates/"
echo "│   ├── config/"
echo "│   └── error/"
echo "├── examples/"
echo "└── (config files)"

echo ""
echo "📋 Copying and fixing files from src_ref/..."

# Function to copy and fix naming in files
copy_and_fix() {
    local src_file="$1"
    local dest_file="$2"
    
    if [ ! -f "src_ref/$src_file" ]; then
        echo "⚠️  Warning: src_ref/$src_file not found, skipping..."
        return
    fi
    
    echo "📄 $src_file → $dest_file"
    # Copy file and replace mini_api_server with nox
    sed 's/mini_api_server/nox/g; s/mini-api-server/nox/g; s/Mini API Server/Nox Server/g' "src_ref/$src_file" > "$dest_file"
}

# Copy and fix core files
echo "🔧 Copying core framework files..."
copy_and_fix "server_lib_rs.rs" "src/lib.rs"
copy_and_fix "server_main_rs.rs" "src/main.rs"
copy_and_fix "server_core_rs.rs" "src/server/mod.rs"
copy_and_fix "server_error_rs.rs" "src/error/mod.rs"
copy_and_fix "server_config_rs.rs" "src/config/mod.rs"
copy_and_fix "build_script.rs" "build.rs"

# Copy plugin system files
echo "🔌 Copying plugin system..."
copy_and_fix "server_plugins_rs.rs" "src/plugins/mod.rs"
copy_and_fix "server_plugin_manager_rs.rs" "src/plugins/manager.rs"
copy_and_fix "server_mock_plugin_rs.rs" "src/plugins/mock.rs"
copy_and_fix "server_health_plugin_rs.rs" "src/plugins/health.rs"

# Copy handler files
echo "🌐 Copying request handlers..."
copy_and_fix "server_handlers_rs.rs" "src/handlers/mod.rs"
copy_and_fix "server_router_rs.rs" "src/server/router.rs"
copy_and_fix "server_static_handler_rs.rs" "src/handlers/static_files.rs"
copy_and_fix "server_proxy_handler_rs.rs" "src/handlers/proxy.rs"

# Copy authentication files
echo "🔐 Copying authentication modules..."
copy_and_fix "server_auth_basic_rs.rs" "src/auth/basic.rs"
copy_and_fix "server_auth_bearer_rs.rs" "src/auth/bearer.rs"
copy_and_fix "server_auth_api_key_rs.rs" "src/auth/api_key.rs"

# Copy session management files
echo "💾 Copying session management..."
copy_and_fix "server_session_rs.rs" "src/session/mod.rs"
copy_and_fix "server_session_memory_rs.rs" "src/session/memory.rs"
copy_and_fix "server_session_file_rs.rs" "src/session/file.rs"
copy_and_fix "server_sqlite_session_rs.rs" "src/session/sqlite.rs"

# Copy adapter files
echo "🔌 Copying adapters..."
copy_and_fix "server_adapters_rs.rs" "src/adapters/mod.rs"
copy_and_fix "server_storage_adapter_rs.rs" "src/adapters/storage.rs"
copy_and_fix "server_redis_adapter_rs.rs" "src/adapters/redis.rs"

# Copy utility files
echo "🛠️ Copying utilities..."
copy_and_fix "server_utils_rs.rs" "src/utils/mod.rs"
copy_and_fix "server_logging_rs.rs" "src/utils/logging.rs"
copy_and_fix "server_templates_rs.rs" "src/templates/mod.rs"

# Copy CLI files
echo "⌨️ Copying CLI interface..."
copy_and_fix "server_cli_rs.rs" "src/cli/mod.rs"
copy_and_fix "server_commands_rs.rs" "src/cli/commands.rs"
copy_and_fix "server_daemon_rs.rs" "src/cli/daemon.rs"

# Copy examples
echo "📚 Copying examples..."
copy_and_fix "integration_example.rs" "examples/integration.rs"
copy_and_fix "usage_example.rs" "examples/basic_usage.rs"

# Create missing module files
echo "📝 Creating missing mod.rs files..."
[ ! -f "src/auth/mod.rs" ] && echo "pub mod basic;\npub mod bearer;\npub mod api_key;" > src/auth/mod.rs

echo ""
echo "✅ Nox Server structure created and files copied!"
echo ""
echo "📋 Files successfully migrated from src_ref/ with naming fixes applied:"
echo "  ✅ mini_api_server → nox"
echo "  ✅ mini-api-server → nox" 
echo "  ✅ Mini API Server → Nox Server"
echo ""
echo "📋 Next steps:"
echo "1. Run 'cargo check' to identify any compilation issues"
echo "2. Fix any remaining import/module issues"
echo "3. Run 'cargo build' to build the server"
echo "4. Test with 'cargo run --bin nox --help'"
echo ""
echo "📁 Key files created:"
echo "  📄 src/lib.rs (main library)"
echo "  📄 src/main.rs (binary entry point)"
echo "  📄 build.rs (build script)"
echo "  🔌 Plugin system in src/plugins/"
echo "  🔐 Authentication in src/auth/"
echo "  💾 Session management in src/session/"
echo "  🌐 Request handlers in src/handlers/"
echo "  ⌨️ CLI interface in src/cli/"
echo ""
echo "🎯 Pro tip: Start with 'cargo check' to see what needs fixing first!"
echo ""
echo "🚀 Ready to build your Nox Server!"
