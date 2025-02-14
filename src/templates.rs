use std::collections::HashMap;
use tera::Tera;

#[derive(Clone)]
pub struct EmailTemplates {
    templates: HashMap<String, String>,
}

impl EmailTemplates {
    pub fn new() -> Result<Self, anyhow::Error> {
        let tera = Tera::new("templates/**/*.html")?;
        
        // Load all templates at startup into memory
        let templates = tera.get_template_names()
            .map(|name| {
                let content = tera.render(name, &tera::Context::new())
                    .expect("Failed to render static template");
                (name.to_string(), content)
            })
            .collect();

        Ok(Self { templates })
    }

    pub fn get(&self, name: &str) -> Option<&String> {
        self.templates.get(name)
    }
}
