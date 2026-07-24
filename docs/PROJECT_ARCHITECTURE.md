# Schema Tools — Project Architecture

This document captures how the `schema-tools` project loads data, processes OpenAPI/JSON-Schema documents, resolves references, runs code generation and feeds the Tera template engine. Use it as a single reference before making changes or adding templates.

## 1. Project layout

The repository is a Cargo workspace with two crates:

- `crates/schematools` — library that does loading, processing, validation and codegen.
- `crates/cli` — thin `clap` wrapper around the library.

```text
schema-tools/
├── Cargo.toml              # workspace definition
├── crates/
│   ├── schematools/        # core library
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── schema.rs        # load JSON/YAML
│   │       ├── storage.rs       # resolve external $ref files
│   │       ├── resolver.rs      # resolve $ref pointers inside a schema
│   │       ├── scope.rs         # naming / path tracking
│   │       ├── tools.rs         # helpers: node traversal, args parsing, filters
│   │       ├── discovery.rs     # template discovery / git registries
│   │       ├── process/         # schema preprocessing
│   │       ├── codegen/         # codegen + Tera integration
│   │       └── validate/
│   └── cli/                # command-line entry point
│       └── src/
│           ├── main.rs
│           └── commands/
│               ├── mod.rs       # shared CLI helpers
│               ├── chain.rs     # chain subcommands
│               ├── codegen.rs   # codegen subcommand
│               ├── process/     # process subcommands
│               ├── registry.rs  # git/local template registries
│               └── validate.rs
```

## 2. Loading data

All input goes through `Schema` in `schema.rs`:

```@crates/schematools/src/schema.rs:9-151```

Key points:

- `Schema::load_url(url)` reads `file://` or `http(s)://` URLs.
- `Schema::load_urls(urls)` loads several files and wraps them into a JSON array; for each loaded body it calls `process::rel_to_absolute_refs`.
- `Schema::from_json(value)` creates an in-memory schema with URL `schema://inline`.
- Format detection:
  - If the URL path ends with `yaml`/`yml`, or the HTTP `Content-Type` contains `yaml`, the file is parsed with `yaml_serde`.
  - Multiple YAML documents are collected into a JSON array.
  - Otherwise `serde_json` is used.
- `path_to_url(path)` converts a CLI path into a `file://` URL; only existing absolute paths succeed.

## 3. Reference resolution

### 3.1 External file resolution — `storage.rs`

`SchemaStorage::new(schema, client)` builds a map of all schemas needed to resolve `$ref`s:

```@crates/schematools/src/storage.rs:12-63```

- Recursively scans `$ref` values from the root schema.
- Loads external files via `Schema::load_url_with_client`.
- Stores every loaded schema keyed by its base URL (fragment stripped).
- Converts every `$ref` inside every loaded schema to an absolute URL.
- Also absolutizes `discriminator.mapping` references.

### 3.2 Pointer resolution — `resolver.rs`

`SchemaResolver` walks a `$ref` and returns the referenced JSON subtree:

```@crates/schematools/src/resolver.rs:12-134```

- `resolve(node, scope, f)` follows the reference through `SchemaStorage`, calls the user callback on the resolved subtree, and pushes/pops the JSON pointer in `SchemaScope`.
- `resolve_once` is identical but does not recurse again if the resolved node is itself a `$ref`.

## 4. Scope and naming — `scope.rs`

`SchemaScope` tracks where the extractor currently is inside a schema. It is the single source of model names.

```@crates/schematools/src/scope.rs:15-352```

Important primitives:

- `property`, `entity`, `form`, `definition`, `reference`, `index`, `glue` push scope tokens.
- `namer()` returns a `BasicNamer` that builds PascalCase names from the scope.
- `Spaces` (Tag, Operation, Id, Parameter) are used later to tag models with their endpoint/parameter context.
- `recurse()` detects circular references.
- `path()` reconstructs the JSON pointer path of the last `$ref`.

## 5. Processing pipeline — `process/`

The CLI `process` command lives in `crates/cli/src/commands/process/mod.rs`:

```@crates/cli/src/commands/process/mod.rs:38-297```

Supported operations:

- `dereference` — replaces `$ref`s with inline schema fragments.
- `merge-all-of` — merges `allOf` into single objects.
- `name` — generates missing schema titles and operation IDs.
- `merge-openapi` / `bump-openapi` — merge/bump OpenAPI specs (feature-gated).
- `patch` — apply JSON patches.

Each processor gets a mutable `Schema` and a `SchemaStorage` and mutates `schema.body` in place. Chain mode reuses the same storage across steps.

## 6. Codegen pipeline

### 6.1 Extraction — `codegen/jsonschema/mod.rs`

For JSON-Schema input, `codegen::jsonschema::extract(schema, storage, options)` returns a `ModelContainer`.

```@crates/schematools/src/codegen/jsonschema/mod.rs:170-206```

- `extract_type` is the main recursive function. It converts schema nodes into `Model` objects.
- `add_types` adds the extracted model to the `ModelContainer`.
- `ModelContainer` holds all models plus a `regexps` list and a `formats` list.
- Duplicate models are detected by hashing the normalized schema. Before hashing, `title` and `description` keys are stripped so purely descriptive differences do not produce separate models.

Model types are defined in `codegen/jsonschema/types.rs`:

```@crates/schematools/src/codegen/jsonschema/types.rs:8-255```

### 6.2 OpenAPI extraction — `codegen/openapi/mod.rs`

For OpenAPI input, `codegen::openapi::extract(schema, storage, options)` returns an `Openapi` struct.

```@crates/schematools/src/codegen/openapi/mod.rs:120-283```

It walks these sections:

1. `components/securitySchemes` → `security::SecuritySchemes`
2. `security` defaults
3. `components/schemas` → models
4. `components/parameters` → models
5. `components/responses` → models
6. `components/requestBodies` → models
7. `paths` → `endpoint::extract_endpoints`

`Endpoint` extraction is in `codegen/openapi/endpoint.rs`:

```@crates/schematools/src/codegen/openapi/endpoint.rs:17-180```

It parses each HTTP method, parameters, request body and responses.

### 6.3 JSON-Schema model extraction rules

`extract_type` in `codegen/jsonschema/mod.rs` is the dispatcher. The full decision tree:

```@crates/schematools/src/codegen/jsonschema/mod.rs:220-352```

1. **Reference resolution first** — if the node is a `$ref`, `SchemaResolver` resolves it.
2. **Already extracted?** — `ModelContainer::resolve(scope.path())` returns a cached model.
3. **Circular reference?** — detected by `scope.recurse()`; returns `AnyType`.
4. **If `type` is present**:
   - `"object"` → `properties::from_object` (tries `properties`, then `patternProperties`, then `additionalProperties`, then `AnyType`).
   - `"array"` → `items::from_array`. **Tuples are not supported** and become `AnyType`.
   - anything else → try `const_::from_const`, then `PrimitiveType`; then `enum_::convert_to_enum`.
   - If the schema also contains `oneOf` or `anyOf`, `anyoneof::from_one_or_any_of` takes precedence over plain enum conversion.
   - If `type` is an array (e.g. `["null","string"]`), `simplify_type` removes `"null"`, sets `nullable=true`, and either keeps the remaining single type or creates a `oneOf` with each remaining type.
5. **If `type` is absent**, the extractor tries in order:
   - `oneOf` / `anyOf`
   - `allOf`
   - `patternProperties`
   - `const`
   - fallback `AnyType`

`add_validation_and_nullable` then attaches:

- JSON-Schema validation keywords: `format`, `maximum`, `exclusiveMaximum`, `minimum`, `exclusiveMinimum`, `maxLength`, `minLength`, `pattern`, `maxItems`, `minItems`, `uniqueItems`, `maxProperties`, `minProperties`, `default`.
- `x-*` extension properties.
- `nullable`, `description`.
- The original schema object when `keep_schema` matches.
- `pattern` is replaced by a reference to a generated `RegexpType`.

### 6.4 `oneOf` / `anyOf` and union handling

```@crates/schematools/src/codegen/jsonschema/anyoneof/mod.rs:16-96```

- **Single variant** → collapses to that variant.
- **Two variants with `null`** → extracts the non-null variant and marks it `nullable`.
- **Otherwise** → creates a `WrapperType` (kind `OneOf`) whose members are named `Variant0`, `Variant1`, …
- If a `discriminator` exists, a dedicated extractor is used; otherwise `Simple` extractor auto-detects tagging:
  - Object variant with exactly one property → **externally** tagged.
  - Object variant with a `const` property → **internally** tagged (`Internally(property)` strategy).
  - Otherwise → `BruteForce` strategy.
- Discriminator metadata is stored inside each variant's `attributes.x["_discriminator"]`.

```@crates/schematools/src/codegen/jsonschema/anyoneof/extractor.rs:100-197```

### 6.5 `allOf` handling

`allof.rs` extracts every member, flattens it, marks it required, names it `Variant{i}`, and produces a `WrapperType` of kind `AllOf`.

```@crates/schematools/src/codegen/jsonschema/allof.rs:16-56```

### 6.6 Optional/nullable wrapping

When `--optional-and-nullable-as-models` is enabled, a property that is both `nullable` and not `required` is wrapped in a separate `NullableOptionalWrapperType` model named like `<Parent><Property>Optional`.

```@crates/schematools/src/codegen/jsonschema/properties.rs:38-47```

### 6.7 Model naming and deduplication

`ModelContainer::add` is the source of truth:

```@crates/schematools/src/codegen/jsonschema/mod.rs:59-109```

- Key for caching is `scope.path()` (the current JSON pointer / reference path).
- If the same pointer was already extracted, the existing model is reused.
- If an equivalent model already exists (`is_like` comparison), its ID is reused (deduplication).
- If a name collision occurs, the new model is renamed with `bump_suffix_number` (`Name2`, `Name3`, …) and re-inserted.

### 6.8 OpenAPI endpoint extraction details

`endpoint::extract_endpoints` does the following:

```@crates/schematools/src/codegen/openapi/endpoint.rs:37-86```

- The path item object is resolved (supports `$ref`).
- Path-level `parameters` are extracted first, then merged into each method's parameters.
- For each HTTP method, `operationId` is taken as-is if present; otherwise generated from path/method/version by `process::name::endpoint`.
- Tags default to `["default"]` if missing.
- All `x-*` keys are collected into `endpoint.x`.
- Operation-level `security` overrides global security.
- Request body and responses are extracted.

Response model metadata:

```@crates/schematools/src/codegen/openapi/responses.rs:36-155```

- `success` is the first 2xx response.
- `is_unique` is true when a model appears in exactly one response/content-type within the endpoint.
- `vnd` is parsed from `application/vnd.<name>+<suffix>`.
- `alternative_content_type` is true for non-default content types when multiple are present.

### 6.9 Current options with limited/no implementation

Two flags are declared in `JsonSchemaExtractOptions` / `OpenapiExtractOptions` but are **not yet wired into the extraction logic**:

- `wrappers`
- `nested_arrays_as_models`

Only `optional_and_nullable_as_models` is currently implemented.

## 7. Processing commands in detail

### 7.1 Dereference

`Dereferencer` recursively replaces `$ref` nodes with the resolved value:

```@crates/schematools/src/process/dereference.rs:61-69```

Options:

- `skip_root_internal_references` — leave `#/...` references at depth 1 untouched.
- `create_internal_references` — when the same external reference is resolved multiple times, later occurrences become `#/pointer` references instead of being inlined again.
- `skip_references` — list of hostnames/URL substrings to leave as references.
- Protects against infinite recursion with a depth limit of 50.
- After resolving a ref, sibling keywords from the original `$ref` object are merged into the resolved object.
- `discriminator.mapping` is also dereferenced when `skip_discriminators` is false.

### 7.2 Merge allOf

`merge_allof::Merger` walks the schema and merges `allOf` arrays into a single object:

```@crates/schematools/src/process/merge_allof.rs:25-33```

- `leave_invalid_properties` controls whether invalid `allOf` content is kept.
- `filter` can restrict which `allOf` nodes are processed.
- Object members are deep-merged; array members are unioned (duplicates removed); other types are overwritten.
- Empty `allOf` and single-element `allOf` are handled as no-ops/warnings.

### 7.3 Naming

`OpenapiNamer` runs over:

```@crates/schematools/src/process/name/openapi.rs:49-164```

- `components/schemas` → adds `title` when missing.
- `components/responses/*/content/*/schema` → adds title.
- `components/requestBodies/*/content/*/schema` → adds title.
- `paths/*/{method}` → adds `operationId` when missing or when `overwrite` is set.

`JsonSchemaNamer` (`process name` for plain JSON-Schema) recursively names `properties`, `definitions`/`$defs`, `items`, `oneOf`, `allOf`, `anyOf`, `not`.

Title generation skips simple non-object types unless `overwrite` is set, and skips ambiguous `oneOf`/`anyOf` members unless `overwrite_ambiguous` is set.

```@crates/schematools/src/process/name/jsonschema.rs:70-194```

Operation ID generation from path/method:

```@crates/schematools/src/process/name/endpoint.rs:62-120```

- Version prefix `vN` is detected and moved to the front.
- `GET /resource` → `listResource(s)`; `GET /resource/{id}` → `getResource`.
- `POST` → `create`; `PATCH` → `update`; other methods use their name.
- Pluralization/singularization is applied based on whether an identifier follows the resource.
- `--resource-method-version` reverses the order to `resourceVerbVersion`.

### 7.4 Merge OpenAPI

`merge_openapi::Merger` merges a second OpenAPI spec into the first:

```@crates/schematools/src/process/merge_openapi.rs:24-147```

- Components are merged without overwriting existing names.
- Paths/methods are merged without overwriting; `retag` adds a single tag to every merged path.
- `add_version` adds `info.x-version-<label>` from the merged spec.
- Tags are unioned by name.

### 7.5 Bump OpenAPI

`bump_openapi::Bumper` compares `info.x-version-*` semver fields between the original spec and the current spec and bumps the root `info.version` accordingly:

```@crates/schematools/src/process/bump_openapi.rs:35-108```

- If any merged service had a major bump → root major bump.
- Else if any minor bump → root minor bump.
- Else if any patch bump → root patch bump.

### 7.6 Patch

`process patch` supports three actions (feature `json-patch`):

```@crates/schematools/src/process/patch.rs:52-79```

- `create <original-file>` — computes a JSON-Patch diff against the original.
- `apply <patch-file>` — applies a JSON-Patch file.
- `inline` — applies a single inline add/remove/replace operation.

## 8. Template system

### 7.1 Discovering templates — `discovery.rs`

`Discovery::resolve(&[template_path])` walks directories or git registries:

```@crates/schematools/src/discovery.rs:21-83```

- Paths can be plain directories or `registry::sub/path/`.
- Every file ending in `.j2` is read as a Tera template, keyed by its relative path.
- Other files are treated as static assets and copied to the output.
- Multiple template paths are processed in order; if the same relative filename exists in more than one path, the file from the last path wins.
- Git registries are checked out into a temp cache via `discover_git`.

### 7.2 Template kinds — `codegen/templates.rs`

Each `.j2` file **must** start with a Tera comment header:

```
{# type=models,filename=models.go #}
{# type=endpoints,filename=endpoints.rs,group_by=tag,if=%options.type%:server #}
```

Header keys handled by the tool:

- `type` — `models`, `endpoints`, `tags`, `static`
- `filename` — output path; may contain `%options.X%` placeholders
- `if` — condition like `options.type:server`; rendered only when left == right
- `group_by` — only `tag` is supported for endpoints
- `content_type` — default `application/json`; used by endpoints to pick response/request models
- `min_version` — minimum schema-tools version required by the template

```@crates/schematools/src/codegen/templates.rs:221-269```

### 7.3 Rendering — `renderer.rs` + `templates.rs`

`codegen::renderer::create` builds a `Tera` instance and parses templates:

```@crates/schematools/src/codegen/renderer.rs:11-34```

Then it calls either `Renderer::models()` or `Renderer::openapi()`:

```@crates/schematools/src/codegen/renderer.rs:36-110```

Actual render is in `process_render`:

```@crates/schematools/src/codegen/templates.rs:624-660```

What is put into the Tera context:

1. The serializable data object (`ModelContainer` for models, `Openapi` for OpenAPI, `{}` for static).
2. Every top-level key of `CodegenContainer` (i.e. `options` plus any `data` entries set by the renderer such as `formats`, `tags`).

This is why templates see `{{ options.name }}`, `{{ models }}`, `{{ endpoints }}`, `{{ tags }}`, etc. at the root.

### 7.4 Custom filters/functions — `filters.rs`

```@crates/schematools/src/codegen/filters.rs:68-100```

Registered filters:

- case filters: `camelcase`, `pascalcase`, `snakecase`, `upper_snakecase`, `kebabcase`, `traincase`, `titlecase`, `lcfirst`, `ucfirst`, `nospaces`
- array filters: `filter_not`, `filter_startswith`, `filter_inarray`, `filter_not_inarray`, `plural`
- string/path filters: `path_parts`, `when_numeric`

Functions:

- `get_bucket_count(bucket="...", name="...")` / `clear_bucket(bucket="...")` — per-render counters used to avoid duplicate names.

## 8. CLI entry and chain mode

`crates/cli/src/main.rs` routes subcommands:

```@crates/cli/src/main.rs:30-49```

`chain` is the main workflow engine:

```@crates/cli/src/commands/chain.rs:96-187```

Steps:

1. Parse each `-c '...'` command.
2. Load the schemas declared by those commands.
3. Build shared `SchemaStorage` from all loaded root schemas.
4. Run the commands sequentially on each schema, reusing storage.
5. Special commands:
   - `registry add` registers a template source.
   - `output` dumps the current schema body.
   - `codegen` reads the current schema.

`codegen` subcommand is in `crates/cli/src/commands/codegen.rs`:

```@crates/cli/src/commands/codegen.rs:148-260```

It loads the schema, creates storage, discovers templates, runs extraction, creates the renderer and renders.

## 9. Shared CLI helpers

`crates/cli/src/commands/mod.rs` provides:

- `get_options` parser for `-o key=value` and `-o 'key=~["a","b"]'`.
- `Verbosity` / logger setup.
- `Output` helper for JSON/YAML stdout or file output.

```@crates/cli/src/commands/mod.rs:31-114```

## 11. Important file index

| Concern | Primary files |
|---|---|
| Loading JSON/YAML | `crates/schematools/src/schema.rs` |
| External `$ref` resolution | `crates/schematools/src/storage.rs` |
| Inline `$ref` resolution | `crates/schematools/src/resolver.rs` |
| Naming / path tracking | `crates/schematools/src/scope.rs` |
| Node traversal helpers | `crates/schematools/src/tools.rs` |
| Preprocessing | `crates/schematools/src/process/mod.rs`, `process/dereference.rs`, `process/merge_allof.rs`, `process/name/` |
| JSON-Schema → models | `crates/schematools/src/codegen/jsonschema/mod.rs`, `codegen/jsonschema/types.rs`, `codegen/jsonschema/properties.rs` |
| OpenAPI → models/endpoints | `crates/schematools/src/codegen/openapi/mod.rs`, `codegen/openapi/endpoint.rs`, `codegen/openapi/parameters.rs`, `codegen/openapi/requestbody.rs`, `codegen/openapi/responses.rs`, `codegen/openapi/security.rs` |
| Template discovery | `crates/schematools/src/discovery.rs` |
| Template parsing / render | `crates/schematools/src/codegen/templates.rs`, `codegen/renderer.rs` |
| Template filters | `crates/schematools/src/codegen/filters.rs` |
| CLI commands | `crates/cli/src/main.rs`, `crates/cli/src/commands/chain.rs`, `crates/cli/src/commands/codegen.rs`, `crates/cli/src/commands/process/mod.rs` |
