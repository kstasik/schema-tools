# Schema Tools — Template Variables Reference

This document lists every variable available inside Tera templates. Use it as a reference when writing templates, e.g. an OpenAPI client generator.

For general Tera syntax and built-in filters/functions, see the [Tera documentation](https://keats.github.io/tera/). If you are migrating templates from Tera v1, consult the [Tera v1 → v2 migration guide](https://github.com/Keats/tera/blob/master/MIGRATION.md).

## 1. How the Tera context is built

Before a template is rendered the tool serializes the data object (`ModelContainer` or `Openapi`) and merges the `CodegenContainer` keys on top of it:

```@crates/schematools/src/codegen/templates.rs:624-660```

Resulting root context:

- `options` — map of values passed via `-o key=value` / `-o 'key=~["a"]'`.
- `models` — available in `type=models` templates and in `type=endpoints`/`type=tags` templates (as `openapi.models`).
- `endpoints` — available in `type=endpoints` templates.
- `security` — available in `type=endpoints` templates.
- `tags` — available in `type=tags` templates.
- `formats` / `tag` — inserted by some renderers for endpoint/tag grouping.

### 1.1 Template loading order

Multiple `--template` directories can be passed to the codegen command. Templates and static files are discovered in the order the directories are provided, keyed by their relative path. If the same relative filename exists in more than one directory, the file from the **last** directory wins and overwrites the earlier one.

## 2. `options` map

Built by `codegen::create_container`:

```@crates/schematools/src/codegen/mod.rs:12-27```

All `-o` pairs become keys in `options`. Use them in file names and conditions:

```jinja2
{# type=endpoints,filename=%options.namespace%/client.rs #}
```

## 3. Models templates (`type=models`)

### 3.1 Root variables

```text
models.regexps   → Vec<RegexpType>
models.formats   → Vec<String>
models.models    → Vec<Model>
options          → Map<String, Value>
```

`ModelContainer` serialization:

```@crates/schematools/src/codegen/jsonschema/mod.rs:34-45```

### 3.2 `Model` object structure

A model is serialized with its inner type flattened, plus `attributes` and `spaces`:

```@crates/schematools/src/codegen/jsonschema/types.rs:8-17```

Possible `type` values and their fields:

| `type` | Rust variant | Object fields |
|---|---|---|
| `primitive` | `PrimitiveType` | `{ name?: string, type: string }` |
| `object` | `ObjectType` | `{ name: string, properties: [FlatModel], additional: bool }` |
| `array` | `ArrayType` | `{ name?: string, model: FlatModel }` |
| `enum` | `EnumType` | `{ name: string, type: string, options: [string] }` |
| `const` | `ConstType` | `{ name: string, type: string, value: string }` |
| `any` | `AnyType` | `{}` |
| `wrapper` | `WrapperType` | `{ name: string, models: [FlatModel], kind: "OneOf"\|"AllOf", strategy: "BruteForce"\|{"Internally":string}\|"Externally" }` |
| `optional` | `NullableOptionalWrapperType` | `{ name: string, model: FlatModel }` |
| `map` | `MapType` | `{ name?: string, model: FlatModel }` |

Model type definitions:

```@crates/schematools/src/codegen/jsonschema/types.rs:46-228```

### 3.3 `attributes` on every model

```text
{
  description: string | null,
  default:     any | null,
  nullable:    bool,
  required:    bool,
  reference:   bool,
  validation:  { [key: string]: any } | null,
  schema:      object | null,        // original JSON-Schema if keep_schema matched
  x:           { [key: string]: any } // all x-* extensions
}
```

Definition:

```@crates/schematools/src/codegen/jsonschema/types.rs:229-254```

### 3.4 `FlatModel` — nested/reference shape

Used for properties, array items, map values, wrapper members, etc. Serialization:

```@crates/schematools/src/codegen/jsonschema/types.rs:396-413```

```text
{
  name:        string | null,
  type:        string,          // "primitive" | "object" | "array" | "enum" | "const" | "wrapper" | "map" | "any" | concrete JSON-Schema type
  model:       FlatModel | null,
  required:    bool,
  nullable:    bool,
  validation:  { [key: string]: any } | null,
  x:           { [key: string]: any },
  description: string | null,
  default:     any | null
}
```

For an `object` property, `type` is `"object"` and `model` contains `{ name, type: "<GeneratedName>" }` plus `reference: true` in attributes (because it points to a model already in the container).

For an `array` property, `type` is `"array"` and `model` is the element `FlatModel`.

For an `enum` property, `type` is `"enum"` and `model` is `{ name, type, required: true }`.

### 3.5 `regexps`

```text
{
  regexps: [
    { name: "Regexp1", pattern: "..." },
    ...
  ]
}
```

## 4. Endpoints templates (`type=endpoints`)

### 4.1 Root variables

```text
endpoints  → [Endpoint]
models     → ModelContainer
security   → SecuritySchemes
tags       → [string]       (global list)
formats    → [string]       (list of formats used)
options    → Map
```

When `group_by=tag` is used, the renderer also adds `tag` (PascalCased) to the context and filters endpoints per tag.

### 4.2 `Endpoint` object

```@crates/schematools/src/codegen/openapi/endpoint.rs:17-29```

```text
{
  security:    [SecurityScheme],
  path:        string,
  method:      string,       // lowercase: get, post, ...
  operation:   string,       // operationId
  description: string | null,
  tags:        [string],
  parameters:  Parameters,
  requestbody: RequestBody | null,
  responses:   Responses,
  x:           { [key: string]: any }
}
```

### 4.3 `Parameters` object

```@crates/schematools/src/codegen/openapi/parameters.rs:15-55```

```text
{
  path:   [Parameter],
  query:  [Parameter],
  header: [Parameter],
  cookie: [Parameter],
  all:    [Parameter]
}
```

### 4.4 `Parameter` object

```@crates/schematools/src/codegen/openapi/parameters.rs:33-55```

```text
{
  model:       FlatModel | null,
  required:    bool,
  name:        string,
  description: string | null,
  style:       string | null,
  explode:     bool | null,
  kind:        "path" | "query" | "header" | "cookie"
}
```

### 4.5 `RequestBody` object

```@crates/schematools/src/codegen/openapi/requestbody.rs:11-21```

```text
{
  models:      MediaModelsContainer | null,
  required:    bool,
  description: string | null
}
```

### 4.6 `Responses` object

```@crates/schematools/src/codegen/openapi/responses.rs:17-34```

```text
{
  success: Response | null,  // first 2xx response
  all:     [Response]
}
```

### 4.7 `Response` object

```text
{
  statusCode: uint32,
  models:      MediaModelsContainer | null,
  description: string | null,
  headers:     [Parameter] | null
}
```

### 4.8 `MediaModelsContainer`

```@crates/schematools/src/codegen/openapi/mod.rs:77-109```

```text
{
  default:   MediaModel | null,
  all:       [MediaModel],
  multipleContentTypes: bool
}
```

### 4.9 `MediaModel`

```@crates/schematools/src/codegen/openapi/mod.rs:37-57```

```text
{
  model: FlatModel,
  contentType: string,
  isUnique: bool,
  alternativeContentType: bool,
  vnd: { base: string, vnd: string } | null
}
```

## 5. Security schemes

### 5.1 Root `security` object

```@crates/schematools/src/codegen/openapi/security.rs:5-26```

```text
{
  default: [SecurityScheme],  // global security requirement
  all:     [SecurityScheme]   // all declared schemes
}
```

### 5.2 `SecurityScheme`

```@crates/schematools/src/codegen/openapi/security.rs:28-45```

```text
{
  scheme_name: string,  // key from components/securitySchemes
  type:        string,  // http, apiKey, oauth2, openIdConnect
  scheme:      string | null,
  in:          string | null,
  name:        string | null
}
```

## 6. Tags templates (`type=tags`)

Root variables:

```text
tags      → [TagContainer]
models    → ModelContainer (with endpoints cleared)
formats   → [string]
options   → Map
```

`TagContainer`:

```@crates/schematools/src/codegen/templates.rs:66-70```

```text
{
  tag:       string,        // PascalCased tag name
  endpoints: [Endpoint]
}
```

## 7. Static templates (`type=static`)

Only `options` is available. The template content itself can still access custom functions/filters.

## 8. Custom filters and functions

Filters are registered in `filters.rs`:

```@crates/schematools/src/codegen/filters.rs:68-100```

| Filter | Example | Description |
|---|---|---|
| `camelcase` | `{{ "foo_bar" \| camelcase }}` → `fooBar` | |
| `pascalcase` | `{{ "foo_bar" \| pascalcase }}` → `FooBar` | |
| `snakecase` | `{{ "FooBar" \| snakecase }}` → `foo_bar` | |
| `upper_snakecase` | `{{ "FooBar" \| upper_snakecase }}` → `FOO_BAR` | |
| `kebabcase` | `{{ "FooBar" \| kebabcase }}` → `foo-bar` | |
| `traincase` | `{{ "foo_bar" \| traincase }}` → `Foo-Bar` | |
| `titlecase` | `{{ "foo_bar" \| titlecase }}` → `Foo Bar` | |
| `lcfirst` | `{{ "Foo" \| lcfirst }}` → `foo` | |
| `ucfirst` | `{{ "foo" \| ucfirst }}` → `Foo` | |
| `nospaces` | `{{ "a b" \| nospaces }}` → `ab` | |
| `plural` | `{{ "order" \| plural }}` → `orders` | |
| `path_parts` | `{{ path \| path_parts(to="{id}") }}` | replaces `{param}` with literal |
| `when_numeric` | `{{ name \| when_numeric(prefix="_") }}` | prefixes names starting with digit |
| `filter_not` | `{{ list \| filter_not(attribute="kind", value="query") }}` | keep items where attribute != value |
| `filter_startswith` | `{{ list \| filter_startswith(attribute="name", value="X", match=true) }}` | keep/remove by prefix |
| `filter_inarray` | `{{ list \| filter_inarray(attribute="operation", values=["a"]) }}` | keep if attribute in values |
| `filter_not_inarray` | inverse of `filter_inarray` | |

Functions:

- `get_bucket_count(bucket="...", name="...")` — returns null the first time a name is seen, then `2`, `3`, … per bucket.
- `clear_bucket(bucket="...")` — resets a bucket.

## 9. Template file name placeholders

`filename=` supports `%options.X%` substitution (dotted keys become JSON pointers):

```@crates/schematools/src/codegen/templates.rs:98-101```

```jinja2
{# type=endpoints,filename=clients/%options.name%/client.rs #}
```

## 10. Minimal OpenAPI client template example

```jinja2
{# type=endpoints,filename=client.rs #}
use reqwest::Client;

pub struct {{ options.name | pascalcase }}Client {
    client: Client,
    base_url: String,
}

impl {{ options.name | pascalcase }}Client {
    pub fn new(base_url: String) -> Self {
        Self { client: Client::new(), base_url }
    }

{% for endpoint in endpoints %}
    pub async fn {{ endpoint.operation | snakecase }}(&self{% for p in endpoint.parameters.all %}, {{ p.name | snakecase }}: {{ p.model | type_to_rust }}{% endfor %}) -> Result<(), reqwest::Error> {
        let url = format!("{}{}", self.base_url, {{ endpoint.path | path_parts(to="{}") | snakecase }});
        self.client.{{ endpoint.method }}(url).send().await?.error_for_status()?;
        Ok(())
    }
{% endfor %}
}
```

The example uses only the documented root/context variables (`options`, `endpoints`, filters). For a real template you would also iterate `models.models` to generate structs.

## 11. Worked examples

These snippets were generated by running `cargo run -p schematools-cli -- codegen` against real inputs. They show exactly what keys/values appear in the Tera context.

### 11.1 JSON-Schema — `oneOf` union

Input (simplified):

```json
{
  "type": "object",
  "title": "Garage",
  "properties": {
    "vehicles": {
      "type": "array",
      "items": {
        "oneOf": [
          { "$ref": "#/definitions/motorcycle" },
          { "$ref": "#/definitions/car" }
        ]
      }
    }
  },
  "definitions": {
    "motorcycle": { "type": "object", "required": ["plate","isTankFull"], "properties": { "plate": {"type":"string"}, "isTankFull": {"type":"boolean"} } },
    "car":      { "type": "object", "required": ["plate","doors"],     "properties": { "plate": {"type":"string"}, "doors": {"type":"number"} } }
  }
}
```

Resulting model for `vehicles` (inside the `Garage` object model):

```json
{
  "name": "vehicles",
  "type": "array",
  "model": {
    "name": null,
    "type": "wrapper",
    "model": {
      "name": "GarageVehiclesVariant3",
      "type": "wrapper",
      "model": null,
      "required": true,
      "nullable": false,
      "validation": null,
      "x": {},
      "description": null,
      "default": null
    },
    "required": true,
    "nullable": false,
    "validation": null,
    "x": {},
    "description": null,
    "default": null
  },
  "required": true,
  "nullable": false,
  "validation": null,
  "x": {},
  "description": null,
  "default": null
}
```

The wrapper itself:

```json
{
  "wrapper": {
    "name": "GarageVehiclesVariant3",
    "models": [
      {
        "name": "Variant0",
        "type": "object",
        "model": {
          "name": "GarageVehiclesVariant",
          "type": "GarageVehiclesVariant",
          "model": null,
          "required": true,
          "nullable": false,
          "validation": null,
          "x": {},
          "description": null,
          "default": null
        },
        "required": true,
        "nullable": false,
        "validation": null,
        "x": {},
        "description": null,
        "default": null
      },
      {
        "name": "Variant1",
        "type": "object",
        "model": {
          "name": "GarageVehiclesVariant2",
          "type": "GarageVehiclesVariant2",
          "model": null,
          "required": true,
          "nullable": false,
          "validation": null,
          "x": {},
          "description": null,
          "default": null
        },
        "required": true,
        "nullable": false,
        "validation": null,
        "x": {},
        "description": null,
        "default": null
      }
    ],
    "kind": "OneOf",
    "strategy": "bruteForce"
  }
}
```

### 11.2 JSON-Schema — enum fields

Input (simplified):

```json
{
  "type": "object",
  "title": "ProductPrice",
  "properties": {
    "currencyCode": { "type": "string", "enum": ["AED","AFN","ALL"] },
    "status":       { "type": "number", "enum": [1,2,3,4,5,6,-5] },
    "value":        { "type": "number" }
  }
}
```

Resulting separate enum model:

```json
{
  "enum": {
    "name": "ProductPriceCurrencyCode",
    "type": "string",
    "options": ["AED", "AFN", "ALL"]
  },
  "attributes": {
    "description": null,
    "default": null,
    "nullable": false,
    "required": true,
    "reference": false,
    "validation": null,
    "schema": null,
    "x": {}
  },
  "spaces": [{ "Id": "https://example.com/arrays.schema.json" }]
}
```

In the parent `ProductPrice` object, the property references it:

```json
{
  "name": "currencyCode",
  "type": "enum",
  "model": {
    "name": "ProductPriceCurrencyCode",
    "type": "string",
    "model": null,
    "required": true,
    "nullable": false,
    "validation": null,
    "x": {},
    "description": null,
    "default": null
  },
  "required": true,
  "nullable": false,
  "validation": null,
  "x": {},
  "description": null,
  "default": null
}
```

### 11.3 OpenAPI — query parameter and security

Input path:

```yaml
paths:
  /pets:
    get:
      operationId: listPets
      tags: [pets]
      parameters:
        - name: limit
          in: query
          schema:
            type: integer
      responses:
        "200":
          description: OK
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: "#/components/schemas/Pet"

components:
  securitySchemes:
    bearer:
      type: http
      scheme: bearer

security:
  - bearer: []
```

Resulting endpoint object (first item in `endpoints`):

```json
{
  "security": [
    {
      "scheme_name": "bearer",
      "type": "http",
      "scheme": "bearer",
      "in": null,
      "name": null
    }
  ],
  "path": "/pets",
  "method": "get",
  "operation": "listPets",
  "description": null,
  "tags": ["pets"],
  "parameters": {
    "path": [],
    "header": [],
    "cookie": [],
    "query": [
      {
        "model": {
          "name": "ListPetsLimitQuery",
          "type": "integer",
          "model": null,
          "required": true,
          "nullable": false,
          "validation": null,
          "x": {},
          "description": null,
          "default": null
        },
        "required": false,
        "name": "limit",
        "description": null,
        "style": null,
        "explode": null,
        "kind": "query"
      }
    ],
    "all": [ /* same as query */ ]
  },
  "requestbody": null,
  "responses": {
    "success": {
      "statusCode": 200,
      "models": {
        "default": {
          "model": {
            "name": null,
            "type": "array",
            "model": {
              "name": null,
              "type": "object",
              "model": {
                "name": "Pet",
                "type": "Pet",
                "model": null,
                "required": true,
                "nullable": false,
                "validation": null,
                "x": {},
                "description": null,
                "default": null
              },
              "required": true,
              "nullable": false,
              "validation": null,
              "x": {},
              "description": null,
              "default": null
            },
            "required": true,
            "nullable": false,
            "validation": null,
            "x": {},
            "description": null,
            "default": null
          },
          "contentType": "application/json",
          "isUnique": true,
          "alternativeContentType": false,
          "vnd": null
        },
        "all": [ /* same shape */ ],
        "multipleContentTypes": false
      },
      "description": "OK",
      "headers": null
    },
    "all": [ /* 200 response */ ]
  },
  "x": {}
}
```

### 11.4 OpenAPI — multiple request body content types and `vnd` response

Request body with two content types:

```yaml
requestBody:
  required: true
  content:
    application/json:
      schema:
        $ref: "#/components/schemas/Pet"
    application/xml:
      schema:
        $ref: "#/components/schemas/Pet"
```

Resulting `requestbody`:

```json
{
  "models": {
    "default": {
      "model": { /* object -> Pet reference */ },
      "contentType": "application/json",
      "isUnique": false,
      "alternativeContentType": false,
      "vnd": null
    },
    "all": [ /* default entry */ ],
    "multipleContentTypes": true
  },
  "required": true,
  "description": null
}
```

Response with `application/vnd.pet+json`:

```yaml
responses:
  "200":
    description: OK
    content:
      application/vnd.pet+json:
        schema:
          $ref: "#/components/schemas/Pet"
```

Resulting `models` entry:

```json
{
  "model": { /* object -> Pet reference */ },
  "contentType": "application/vnd.pet+json",
  "isUnique": true,
  "alternativeContentType": false,
  "vnd": {
    "base": "application/json",
    "vnd": "pet"
  }
}
```

### 11.5 OpenAPI — path-level parameter reuse

Path parameter from a `$ref`:

```yaml
parameters:
  petId:
    name: petId
    in: path
    required: true
    schema:
      type: integer

paths:
  /pets/{petId}:
    get:
      parameters:
        - $ref: "#/components/parameters/petId"
```

Resulting `parameters.path[0]`:

```json
{
  "model": {
    "name": "PetIdParameter",
    "type": "integer",
    "model": null,
    "required": true,
    "nullable": false,
    "validation": null,
    "x": {},
    "description": null,
    "default": null
  },
  "required": true,
  "name": "petId",
  "description": null,
  "style": null,
  "explode": null,
  "kind": "path"
}
```

## 12. Useful debugging tip

Inside any template you can dump the available context:

```jinja2
{# type=static,filename=context_dump.json,min_version=0.23.1 #}
{{ __tera_context | safe }}
```

This writes the entire merged context as raw JSON to `context_dump.json`.
