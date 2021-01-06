use crate::error::Error;
use tera::Tera;

pub struct Renderer {
    pub tera: Tera,
    pub templates: super::templates::Templates,
    pub container: super::CodegenContainer,
}

pub fn create(
    templates_dir: &str,
    required: &[super::templates::TemplateType],
    container: super::CodegenContainer,
) -> Result<Renderer, Error> {
    let tera = match Tera::new(&format!(
        "{}{}",
        templates_dir.trim_end_matches('/'),
        "/**/*"
    )) {
        Ok(ref mut t) => Ok(super::filters::register(t)),
        Err(e) => Err(Error::CodegenTemplatesParseError(e)),
    }?;

    let templates = super::templates::get(templates_dir)?;
    if !templates.includes(required) {
        return Err(Error::CodegenMissingRequiredTemplates);
    }

    Ok(Renderer {
        tera,
        templates,
        container,
    })
}

impl Renderer {
    pub fn models(
        &self,
        models: super::jsonschema::ModelContainer,
        target_dir: &str,
        format: &Option<String>,
    ) -> Result<(), Error> {
        let files = self
            .templates
            .list
            .iter()
            .filter(|t| {
                matches!(
                    t,
                    super::templates::Template::Models(_) | super::templates::Template::Static(_)
                )
            })
            .collect::<Vec<_>>();

        for template in files {
            let files = match template {
                super::templates::Template::Static(t) => t.copy(target_dir),
                super::templates::Template::Models(t) => {
                    t.render(&self.tera, target_dir, &models, &self.container)
                }
                _ => Ok(vec![]),
            }?;

            if let Some(command) = format {
                template.format(command, files)?
            }
        }

        Ok(())
    }

    pub fn openapi(
        &self,
        openapi: super::openapi::Openapi,
        target_dir: &str,
        format: &Option<String>,
    ) -> Result<(), Error> {
        let mut files: Vec<Vec<String>> = vec![];

        for template in &self.templates.list {
            files.push(match template {
                super::templates::Template::Static(t) => t.copy(target_dir),
                super::templates::Template::Endpoints(t) => {
                    t.render(&self.tera, target_dir, &openapi, &self.container)
                }
                super::templates::Template::Models(t) => {
                    t.render(&self.tera, target_dir, &openapi.models, &self.container)
                }
            }?);
        }

        if let Some(command) = format {
            for (i, list) in files.iter().enumerate() {
                let template = &self.templates.list.get(i).unwrap();

                template.format(command, list.clone())?
            }
        }

        Ok(())
    }
}
