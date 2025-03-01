use tera::{Tera, Context};

use serde_json::Value;


#[derive(Debug, Clone)]
pub struct EmailTemplates {
    tera: Tera,
}

impl EmailTemplates {
    pub fn new() -> Result<Self, anyhow::Error> {
        let mut tera = Tera::new("templates/**/*.html")?;

        tera.autoescape_on(vec![]);

        if tera.get_template_names().count() == 0 {
            return Err(anyhow::anyhow!("No templates found"));
        }
        
        Ok(Self { tera })
    }

    pub fn render(&self, template_name: &str, data: Value) -> Result<String, anyhow::Error> {

        if !self.tera.templates.contains_key(template_name) {
            return Err(anyhow::anyhow!("Template not found: {}", template_name));
        }
        
        let mut context = Context::new();

        match data {
            Value::Object(obj) => {
                for (k, v) in obj {
                    context.insert(k, &v);
                }
            },
            Value::Null => {
                // Empty context is fine
            },
            _ => {
                context.insert("data", &data);
            }
        }
        
        let rendered = self.tera.render(template_name, &context)?;
        Ok(rendered)
    }
}
