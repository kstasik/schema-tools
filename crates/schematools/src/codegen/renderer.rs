use crate::{discovery::Discovered, error::Error};
use tera::Tera;

pub struct Renderer {
    pub tera: Tera,
    pub templates: super::templates::Templates,
    pub container: super::CodegenContainer,
}

// todo: refactor, it should allocate templates only once if same templates are used
pub fn create(
    discovered: Discovered,
    required: &[super::templates::TemplateType],
    container: super::CodegenContainer,
) -> Result<Renderer, Error> {
    let mut tera = Tera::default();

    // todo: more borrowing, less allocating
    tera.add_raw_templates(discovered.templates.clone())
        .map_err(Error::CodegenTemplatesParseError)?;

    super::filters::register(&mut tera);

    let templates = super::templates::get(discovered)?;
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
                super::templates::Template::File(t) => t.copy(target_dir),
                super::templates::Template::Models(t) => {
                    t.render(&self.tera, target_dir, &models, &self.container)
                }
                super::templates::Template::Static(t) => {
                    t.render(&self.tera, target_dir, &self.container)
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
                super::templates::Template::File(t) => t.copy(target_dir),
                super::templates::Template::Static(t) => {
                    t.render(&self.tera, target_dir, &self.container)
                }
                super::templates::Template::Endpoints(t) => {
                    t.render(&self.tera, target_dir, &openapi, &self.container)
                }
                super::templates::Template::Tags(t) => {
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
