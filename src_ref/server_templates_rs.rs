use crate::error::{ServerError, Result};
use handlebars::{Handlebars, Helper, Context, RenderContext, Output, HelperResult};
use serde_json::Value;

pub struct TemplateEngine {
    handlebars: Handlebars<'static>,
}

impl TemplateEngine {
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();
        
        // Register custom helpers
        handlebars.register_helper("uuid", Box::new(uuid_helper));
        handlebars.register_helper("random", Box::new(random_helper));
        handlebars.register_helper("timestamp", Box::new(timestamp_helper));
        handlebars.register_helper("base64", Box::new(base64_helper));
        handlebars.register_helper("url_encode", Box::new(url_encode_helper));
        handlebars.register_helper("json", Box::new(json_helper));
        handlebars.register_helper("fake_data", Box::new(fake_data_helper));
        
        Self { handlebars }
    }
    
    /// Render a template string with context
    pub fn render_string(&self, template: &str, context: &Value) -> Result<String> {
        self.handlebars
            .render_template(template, context)
            .map_err(ServerError::Template)
    }
    
    /// Register a new template
    pub fn register_template(&mut self, name: &str, template: &str) -> Result<()> {
        self.handlebars
            .register_template_string(name, template)
            .map_err(ServerError::Template)
    }
    
    /// Render a registered template
    pub fn render_template(&self, name: &str, context: &Value) -> Result<String> {
        self.handlebars
            .render(name, context)
            .map_err(ServerError::Template)
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

// Custom helper functions for templates

fn uuid_helper(
    _h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let uuid = uuid::Uuid::new_v4().to_string();
    out.write(&uuid)?;
    Ok(())
}

fn random_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h.param(0);
    
    match param.and_then(|v| v.value().as_str()) {
        Some("int") => {
            let min = h.param(1).and_then(|v| v.value().as_i64()).unwrap_or(0);
            let max = h.param(2).and_then(|v| v.value().as_i64()).unwrap_or(100);
            let random_int = rand::random::<i64>() % (max - min + 1) + min;
            out.write(&random_int.to_string())?;
        }
        Some("float") => {
            let random_float: f64 = rand::random();
            out.write(&random_float.to_string())?;
        }
        Some("bool") => {
            let random_bool: bool = rand::random();
            out.write(&random_bool.to_string())?;
        }
        Some("string") => {
            let length = h.param(1).and_then(|v| v.value().as_u64()).unwrap_or(10) as usize;
            let random_string: String = (0..length)
                .map(|_| {
                    let chars = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
                    chars[rand::random::<usize>() % chars.len()] as char
                })
                .collect();
            out.write(&random_string)?;
        }
        _ => {
            let random_int: u32 = rand::random();
            out.write(&random_int.to_string())?;
        }
    }
    
    Ok(())
}

fn timestamp_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let format = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("rfc3339");
    
    let now = chrono::Utc::now();
    let timestamp = match format {
        "unix" => now.timestamp().to_string(),
        "rfc3339" => now.to_rfc3339(),
        "iso8601" => now.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        custom_format => now.format(custom_format).to_string(),
    };
    
    out.write(&timestamp)?;
    Ok(())
}

fn base64_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    if let Some(input) = h.param(0).and_then(|v| v.value().as_str()) {
        let encoded = base64::encode(input);
        out.write(&encoded)?;
    }
    Ok(())
}

fn url_encode_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    if let Some(input) = h.param(0).and_then(|v| v.value().as_str()) {
        let encoded = urlencoding::encode(input);
        out.write(&encoded)?;
    }
    Ok(())
}

fn json_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    if let Some(value) = h.param(0) {
        let json = serde_json::to_string(value.value()).unwrap_or_default();
        out.write(&json)?;
    }
    Ok(())
}

fn fake_data_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let data_type = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("name");
    
    let fake_value = match data_type {
        "name" => generate_fake_name(),
        "email" => generate_fake_email(),
        "phone" => generate_fake_phone(),
        "address" => generate_fake_address(),
        "company" => generate_fake_company(),
        "lorem" => {
            let words = h.param(1).and_then(|v| v.value().as_u64()).unwrap_or(5) as usize;
            generate_lorem_ipsum(words)
        }
        _ => "Unknown".to_string(),
    };
    
    out.write(&fake_value)?;
    Ok(())
}

// Simple fake data generators (in a real implementation, you might use a crate like 'fake')

fn generate_fake_name() -> String {
    let first_names = ["John", "Jane", "Alice", "Bob", "Charlie", "Diana", "Eve", "Frank"];
    let last_names = ["Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller", "Davis"];
    
    let first = first_names[rand::random::<usize>() % first_names.len()];
    let last = last_names[rand::random::<usize>() % last_names.len()];
    
    format!("{} {}", first, last)
}

fn generate_fake_email() -> String {
    let domains = ["example.com", "test.org", "demo.net", "sample.io"];
    let domain = domains[rand::random::<usize>() % domains.len()];
    let username: String = (0..8)
        .map(|_| {
            let chars = b"abcdefghijklmnopqrstuvwxyz";
            chars[rand::random::<usize>() % chars.len()] as char
        })
        .collect();
    
    format!("{}@{}", username, domain)
}

fn generate_fake_phone() -> String {
    format!(
        "+1-{:03}-{:03}-{:04}",
        rand::random::<u16>() % 900 + 100,
        rand::random::<u16>() % 900 + 100,
        rand::random::<u16>() % 9000 + 1000
    )
}

fn generate_fake_address() -> String {
    let streets = ["Main St", "Oak Ave", "Park Rd", "First St", "Second Ave", "Elm St"];
    let street = streets[rand::random::<usize>() % streets.len()];
    let number = rand::random::<u16>() % 9999 + 1;
    
    format!("{} {}", number, street)
}

fn generate_fake_company() -> String {
    let prefixes = ["Tech", "Digital", "Global", "Smart", "Advanced", "Future"];
    let suffixes = ["Solutions", "Systems", "Corp", "Inc", "Ltd", "Technologies"];
    
    let prefix = prefixes[rand::random::<usize>() % prefixes.len()];
    let suffix = suffixes[rand::random::<usize>() % suffixes.len()];
    
    format!("{} {}", prefix, suffix)
}

fn generate_lorem_ipsum(words: usize) -> String {
    let lorem_words = [
        "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit",
        "sed", "do", "eiusmod", "tempor", "incididunt", "ut", "labore", "et", "dolore",
        "magna", "aliqua", "enim", "ad", "minim", "veniam", "quis", "nostrud",
        "exercitation", "ullamco", "laboris", "nisi", "aliquip", "ex", "ea", "commodo",
        "consequat", "duis", "aute", "irure", "in", "reprehenderit", "voluptate",
        "velit", "esse", "cillum", "fugiat", "nulla", "pariatur", "excepteur", "sint",
        "occaecat", "cupidatat", "non", "proident", "sunt", "culpa", "qui", "officia",
        "deserunt", "mollit", "anim", "id", "est", "laborum"
    ];
    
    (0..words)
        .map(|_| lorem_words[rand::random::<usize>() % lorem_words.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_template_rendering() {
        let engine = TemplateEngine::new();
        let context = json!({
            "name": "World",
            "number": 42
        });
        
        let result = engine.render_string("Hello {{name}}! Number: {{number}}", &context).unwrap();
        assert_eq!(result, "Hello World! Number: 42");
    }
    
    #[test]
    fn test_uuid_helper() {
        let engine = TemplateEngine::new();
        let context = json!({});
        
        let result = engine.render_string("ID: {{uuid}}", &context).unwrap();
        assert!(result.starts_with("ID: "));
        assert!(result.len() > 10);
    }
    
    #[test]
    fn test_random_helper() {
        let engine = TemplateEngine::new();
        let context = json!({});
        
        let result = engine.render_string("Random int: {{random 'int' 1 10}}", &context).unwrap();
        assert!(result.starts_with("Random int: "));
    }
}