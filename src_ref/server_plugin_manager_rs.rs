use super::{Plugin, PluginContext, PluginHook, PluginInfo, PluginResult};
use crate::error::{ServerError, Result};
use hyper::{Request, Response, Body};
use std::collections::HashMap;
use std::sync::Arc;

/// Plugin manager handles registration and execution of plugins
pub struct PluginManager {
    plugins: HashMap<String, Arc<dyn Plugin>>,
    hook_plugins: HashMap<PluginHook, Vec<(String, i32)>>, // (name, priority)
    enabled_plugins: HashMap<String, bool>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            hook_plugins: HashMap::new(),
            enabled_plugins: HashMap::new(),
        }
    }
    
    /// Register a plugin
    pub fn register_plugin(&mut self, plugin: Arc<dyn Plugin>) -> Result<()> {
        let name = plugin.name().to_string();
        let priority = plugin.priority();
        
        // Build hook mapping
        for hook in [
            PluginHook::OnStartup,
            PluginHook::OnShutdown,
            PluginHook::PreRequest,
            PluginHook::PostRoute,
            PluginHook::PreHandler,
            PluginHook::PostHandler,
            PluginHook::PreResponse,
            PluginHook::PostResponse,
            PluginHook::OnError,
        ] {
            if plugin.handles_hook(&hook) {
                let plugins_for_hook = self.hook_plugins.entry(hook).or_insert_with(Vec::new);
                plugins_for_hook.push((name.clone(), priority));
                // Sort by priority (lower numbers first)
                plugins_for_hook.sort_by_key(|(_, p)| *p);
            }
        }
        
        self.enabled_plugins.insert(name.clone(), true);
        self.plugins.insert(name, plugin);
        Ok(())
    }
    
    /// Enable or disable a plugin
    pub fn set_plugin_enabled(&mut self, name: &str, enabled: bool) {
        self.enabled_plugins.insert(name.to_string(), enabled);
    }
    
    /// Check if a plugin is enabled
    pub fn is_plugin_enabled(&self, name: &str) -> bool {
        self.enabled_plugins.get(name).copied().unwrap_or(false)
    }
    
    /// Execute plugins for startup hook
    pub async fn execute_startup(&self, context: &PluginContext) -> Result<()> {
        self.execute_hook_simple(PluginHook::OnStartup, context, |plugin, ctx| {
            Box::pin(plugin.on_startup(ctx))
        }).await
    }
    
    /// Execute plugins for shutdown hook
    pub async fn execute_shutdown(&self, context: &PluginContext) -> Result<()> {
        self.execute_hook_simple(PluginHook::OnShutdown, context, |plugin, ctx| {
            Box::pin(plugin.on_shutdown(ctx))
        }).await
    }
    
    /// Execute plugins for pre-request hook
    pub async fn execute_pre_request(
        &self,
        request: &mut Request<Body>,
        context: &PluginContext,
    ) -> Result<Option<Response<Body>>> {
        self.execute_hook_with_request(PluginHook::PreRequest, request, context, |plugin, req, ctx| {
            Box::pin(plugin.pre_request(req, ctx))
        }).await
    }
    
    /// Execute plugins for post-route hook
    pub async fn execute_post_route(
        &self,
        request: &Request<Body>,
        context: &PluginContext,
    ) -> Result<Option<Response<Body>>> {
        self.execute_hook_simple_with_response(PluginHook::PostRoute, context, |plugin, ctx| {
            Box::pin(plugin.post_route(request, ctx))
        }).await
    }
    
    /// Execute plugins for pre-handler hook
    pub async fn execute_pre_handler(
        &self,
        request: &Request<Body>,
        context: &PluginContext,
    ) -> Result<Option<Response<Body>>> {
        self.execute_hook_simple_with_response(PluginHook::PreHandler, context, |plugin, ctx| {
            Box::pin(plugin.pre_handler(request, ctx))
        }).await
    }
    
    /// Execute plugins for post-handler hook
    pub async fn execute_post_handler(
        &self,
        request: &Request<Body>,
        response: &mut Response<Body>,
        context: &PluginContext,
    ) -> Result<()> {
        if let Some(plugin_names) = self.hook_plugins.get(&PluginHook::PostHandler) {
            for (plugin_name, _) in plugin_names {
                if !self.is_plugin_enabled(plugin_name) {
                    continue;
                }
                
                if let Some(plugin) = self.plugins.get(plugin_name) {
                    match plugin.post_handler(request, response, context).await? {
                        PluginResult::Continue => continue,
                        PluginResult::Stop => break,
                        PluginResult::Error(e) => return Err(e),
                        PluginResult::Response(_) => {
                            // For post-handler, we ignore response results
                            continue;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    
    /// Execute plugins for pre-response hook
    pub async fn execute_pre_response(
        &self,
        response: &mut Response<Body>,
        context: &PluginContext,
    ) -> Result<()> {
        if let Some(plugin_names) = self.hook_plugins.get(&PluginHook::PreResponse) {
            for (plugin_name, _) in plugin_names {
                if !self.is_plugin_enabled(plugin_name) {
                    continue;
                }
                
                if let Some(plugin) = self.plugins.get(plugin_name) {
                    match plugin.pre_response(response, context).await? {
                        PluginResult::Continue => continue,
                        PluginResult::Stop => break,
                        PluginResult::Error(e) => return Err(e),
                        PluginResult::Response(_) => {
                            // For pre-response, we ignore response results
                            continue;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    
    /// Execute plugins for post-response hook
    pub async fn execute_post_response(
        &self,
        response: &Response<Body>,
        context: &PluginContext,
    ) -> Result<()> {
        self.execute_hook_simple(PluginHook::PostResponse, context, |plugin, ctx| {
            Box::pin(plugin.post_response(response, ctx))
        }).await
    }
    
    /// Execute plugins for error hook
    pub async fn execute_on_error(
        &self,
        error: &ServerError,
        context: &PluginContext,
    ) -> Result<Option<Response<Body>>> {
        self.execute_hook_simple_with_response(PluginHook::OnError, context, |plugin, ctx| {
            Box::pin(plugin.on_error(error, ctx))
        }).await
    }
    
    /// Helper method for simple hook execution
    async fn execute_hook_simple<F, Fut>(
        &self,
        hook: PluginHook,
        context: &PluginContext,
        callback: F,
    ) -> Result<()>
    where
        F: Fn(&Arc<dyn Plugin>, &PluginContext) -> Fut,
        Fut: std::future::Future<Output = Result<PluginResult>>,
    {
        if let Some(plugin_names) = self.hook_plugins.get(&hook) {
            for (plugin_name, _) in plugin_names {
                if !self.is_plugin_enabled(plugin_name) {
                    continue;
                }
                
                if let Some(plugin) = self.plugins.get(plugin_name) {
                    match callback(plugin, context).await? {
                        PluginResult::Continue => continue,
                        PluginResult::Stop => break,
                        PluginResult::Error(e) => return Err(e),
                        PluginResult::Response(_) => {
                            // For simple hooks, we ignore response results
                            continue;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    
    /// Helper method for hook execution that can return a response
    async fn execute_hook_simple_with_response<F, Fut>(
        &self,
        hook: PluginHook,
        context: &PluginContext,
        callback: F,
    ) -> Result<Option<Response<Body>>>
    where
        F: Fn(&Arc<dyn Plugin>, &PluginContext) -> Fut,
        Fut: std::future::Future<Output = Result<PluginResult>>,
    {
        if let Some(plugin_names) = self.hook_plugins.get(&hook) {
            for (plugin_name, _) in plugin_names {
                if !self.is_plugin_enabled(plugin_name) {
                    continue;
                }
                
                if let Some(plugin) = self.plugins.get(plugin_name) {
                    match callback(plugin, context).await? {
                        PluginResult::Continue => continue,
                        PluginResult::Stop => break,
                        PluginResult::Error(e) => return Err(e),
                        PluginResult::Response(response) => return Ok(Some(response)),
                    }
                }
            }
        }
        Ok(None)
    }
    
    /// Helper method for hook execution with mutable request
    async fn execute_hook_with_request<F, Fut>(
        &self,
        hook: PluginHook,
        request: &mut Request<Body>,
        context: &PluginContext,
        callback: F,
    ) -> Result<Option<Response<Body>>>
    where
        F: Fn(&Arc<dyn Plugin>, &mut Request<Body>, &PluginContext) -> Fut,
        Fut: std::future::Future<Output = Result<PluginResult>>,
    {
        if let Some(plugin_names) = self.hook_plugins.get(&hook) {
            for (plugin_name, _) in plugin_names {
                if !self.is_plugin_enabled(plugin_name) {
                    continue;
                }
                
                if let Some(plugin) = self.plugins.get(plugin_name) {
                    match callback(plugin, request, context).await? {
                        PluginResult::Continue => continue,
                        PluginResult::Stop => break,
                        PluginResult::Error(e) => return Err(e),
                        PluginResult::Response(response) => return Ok(Some(response)),
                    }
                }
            }
        }
        Ok(None)
    }
    
    /// Get plugin by name
    pub fn get_plugin(&self, name: &str) -> Option<&Arc<dyn Plugin>> {
        self.plugins.get(name)
    }
    
    /// List all registered plugins
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugins
            .values()
            .map(|plugin| {
                let mut info = PluginInfo::from_plugin(plugin.as_ref());
                info.enabled = self.is_plugin_enabled(&info.name);
                info
            })
            .collect()
    }
    
    /// Get plugins for a specific hook
    pub fn plugins_for_hook(&self, hook: &PluginHook) -> Vec<&str> {
        self.hook_plugins
            .get(hook)
            .map(|plugins| {
                plugins
                    .iter()
                    .filter(|(name, _)| self.is_plugin_enabled(name))
                    .map(|(name, _)| name.as_str())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}