# Schema Tools

[![build](https://github.com/kstasik/schema-tools/workflows/build/badge.svg)](https://github.com/kstasik/schema-tools/actions)
[![tests](https://github.com/kstasik/schema-tools/workflows/test/badge.svg)](https://github.com/kstasik/schema-tools/actions)

# Introduction

Just another approach to openapi/jsonschema code generator. It's a home project written in `Rust`, simple all-in-one console tool with features like:

- openapi/json schema validation
- schema preprocessing
    - dereference
    - merge-allof
    - patch (apply/generate json-patch)
    - name
    - merge-openapi
- [Tera](https://github.com/Keats/tera) (jinja2) **code generator** with custom templates support

It is designed to speed up development of mircoservices heavly using json objects (json schemas on api level as well events).

Main differences in approach between other solutions like `openapi-generator`:

- more robust template language like jinja2
- language specific logic moved to templates including type mapping, reserved words
- wrapping of mixed types without discriminator
- one tool for clients, servers, json-schema queue consumers and processing of openapi
- relatively small binary used on every build of project
- codegen executed per microservice approach (not as a separate, generic client library)
- json-schema registry support, TODO: shared models - create one model for shared structures in different clients/servers to avoid mapping same structures

# General rules

- All commands support yaml and json files.
- Use help to get list of available arguments `schema-tools process --help`
- `-v`, `-vv`, `-vvv`, `-vvvv` verbosity levels

# Validate

To validate openapi specification:

```
schematools validate openapi openapi.yaml
```

To validate json schema definition:

```
schematools validate openapi schema.yaml
```

Both commands return non-zero exit code in case of failure. Error reporting is not very clear but it shows the place where json schema is not met. TODO: resolve this [issue](https://github.com/Stranger6667/jsonschema-rs/issues?q=is%3Aissue+is%3Aopen+error)

## Process

Common CLI arguments:

```
<file>                      Path to json/yaml file with openapi specification
-o, --output <output>       Returned format [default: json] [possible values: json,  yaml]
--to-file <to-file>         Path of output file, default output to stdout
```

### Naming

If your openapi specification follows `RESTFUL` openapi rules you can create missing json-schema titles or try to rename operationId of existing endpoint:

```
schematools process name schema.yaml
```

Additional options are:
```
--overwrite                  Should overwrite existing titles
--overwrite-ambiguous         Should overwrite ambiguous titles
--resource-method-version    Reverts order of operationId generator to resource+method+version
```

### Dereferencing

To replace all occurrences of `$ref` in openapi you may type:

```
schematools process dereference schema.yaml
```

You should hardly ever perform full dereference of openapi. It partially dereference schema you may use following options:

- **skip-references** - is useful if you are using registry of common schemas. In many cases you don't want to dereference such `$ref`s
- **skip-root-internal-references** - is useful to skip `/components.*` openapi schema references which are very often stored in root openapi file.
- **create-internal-references** - saves space and dereferences each pointer ony once, all next occurrences are replaced by pointer to first pointer

```
 --create-internal-references             Creates internal references if refs where pointing to same place
--skip-root-internal-references           Leaves internal references intact in root schema file
--skip-references <skip-references>...    List of hostnames to skip dereference
```

### Merge all of

To merge `allOf`s into objects type:

```
schematools process merge-all-of openapi.yaml
```

It's useful to perform such thing before code generation taking into account that json schema is more represntation of validation not data structure itself. In many languages unions are a complicated thing but if you are using allOfs to extract common parts of structs it's a functionality which may be very helpful for you.

### Patch

If openapi you received seems broken you may fix it and create [json-patch](http://jsonpatch.com/) file:

```
schematools process patch <file> create <original-file> 
```

Then you can apply such patch to original openapi file during processing:

```
schematools process patch <file> apply <patch-file> 
```

### Merge openapi and bump

If you microservice is splitted to more than one service (and is exposed under same ingress) you may find it useful to create one openapi definition:

```
schematools process merge-openapi <file> --with <with>
```

Some useful options which may be needed for versioning merged openapi:

```
--add-version <add-version>    Should add info.x-version- attribute to openapi specification
--retag <retag>                Should change tags of all endpoints of merged openapi
```

To bump merged openapi version you may use this command:

```
schematools process bump-openapi <file> --original <previous-version-file>
```

It should correctly change version of openapi according to all sub-openapi semversions.

## Codegen openapi

Code generation itself is performed by processing templates directory. Before it is done all data from openapi/json-schema files has to be extracted and processed. There are two ways of performing codegen:

- `schematools codegen json-schema json-schema.json [...]` is used to process json schema file. It needs additional attribute `--base-name <base-name>` if title of json schema is missing. One json-schema doesnt mean that result of such codegeneration run will be exactly one struct/object/class - in case of complex json schemas it will be many models.
- `schematools codegen openapi openapi.json [...]` is used to process openapi specification - endpoints and models extraction.

Simple usage:

```
schema-tools codegen openapi openapi.json --template templates/  --target-dir pkg/client/
```

- `openapi.json` - openapi specification file
- `--template templates/` - directory with jinja2 files
- `--target-dir pkg/client/` - where code should be generated

### Codegen options

- `--nested-arrays-as-models` - some languages allow to create `Vec<HashMap<Vec<HashMap>>>>` / `[][][]int` inline types, some may need to create wrapping types for such cases
- `--optional-and-nullable-as-models` - openapi allows to create two levels of "nullability", some languages doesnt distinguish between null and undefined. This option wrap all occurences of nullable and optional fields in separate types
- `--wrappers` - option to wrap mixed types (oneOf) to custom objects with custom deserialization logic
- `-o <options>` - option to pass options (string or json) to all templates files ex. `-o 'name=ordersClient' -o 'usedEndpoints=["/orders", "/orders/{id}/items"]'`
- `--format` - executes language formatter after code generation ex. `--format "gofmt -w"`

### Codegen templates

Codegen templates directory is targeted using `--template templates/` option. All simple files from this directory are copied to `--target-dir` beside `.j2` templates. The most important of each `.j2` is it's header (first line of file).

Example of models.j2:

```
{# type=models,filename=models.go #}

/* options: {{ options | json_encode(pretty=true) }} */
/* models: {{ models | json_encode(pretty=true) }} */
```

Example of endpoints.j2:

```
{# type=endpoints,filename=endpoints.go #}

/* options: {{ options | json_encode(pretty=true) }} */
/* models: {{ endpoints | json_encode(pretty=true) }} */
```

*Header* decides how to treat template file, how to and when generate files. Header options:

- `type=?` - possible values: `endpoints`, `models`
- `filename=?` - target filepath to create. May be mixed with options, ex. `filename=clients/%options.name%/endpoints.go`
- `if=foo:bar` - condition when to use template file. Should be mixed with options ex. `if=%options.type%:server`

For more information how to write template files please refer to [Tera docs](https://tera.netlify.app/docs/). To get list of additional filters we created please visit [filters.rs](https://github.com/kstasik/schema-tools/blob/master/src/codegen/filters.rs).

### Codegen template inheritance

Codegen allows to defined multiple `--template` options.

```
schematools codegen openapi.json --template dir1/ --template2 dir2/ --target-dir output/
```

Files from all directories are loaded one by one and in case of conflicts they are overwritten. There is also option to point to registry which currently may be only a **git repository**:

```
schematools codegen openapi.json --template REGISTRY::dir1/ --template2 dir2/ --target-dir output/
```

### Codegen ready to use templates

TODO: push codegen templates to the separate repo and write there an example

## Chain

This is the whole point of this tool. It wraps all existing functionalities together and adds global application context to openapi processing.

```
schematools chain -vvvv \
   -c 'process merge-all-of --leave-invalid-properties https://domain.com/openapi/orders/api.yaml' \
   -c 'process name - --resource-method-version --overwrite' \
   -c 'validate openapi - --continue-on-error' \
   -c 'codegen openapi - \
        --template codegen/client/ \
        --format "gofmt -w" \
        --target-dir pkg/client/ \
        -o namespace=orders \
        -o clientName=OrdersClient'
```

All commands take same arguments as they were executed separately. The only difference is that the first execution has to take real schema file as `-f` argument. The next executions should take `-` to use previously generated schema file.

```
schematools chain -vvvv \
   -c 'process merge-all-of --leave-invalid-properties specifications/api.yaml' \
   -c 'process name - --resource-method-version --overwrite' \
   -c 'validate openapi - ' 
   -c 'codegen openapi - \
        --template codegen/server/ \
        --format "gofmt -w" \
        --target-dir internal/http/ \
        -o namespace=myservice'
```

There is an option to dump processed schema to a file during chaining using `output` command:

```
schematools chain -vvvv \
   -c 'process merge-all-of --leave-invalid-properties specifications/api.yaml' \
   -c 'process name - --resource-method-version --overwrite' \
   -c 'validate openapi - ' \
   -c 'output --to-file=test.json -o json' \
   -c 'codegen openapi - \
        --template codegen/server/ \
        --format "gofmt -w" \
        --target-dir internal/http/ \
        -o namespace=myservice'
```

### Registry

There is an option to treat a separate git repository as source of templates:

```
schematools chain -vvvv \
   -c 'registry add common git://github.com/kstasik/schema-tools --tag v0.0.1' \
   -c 'process merge-all-of --leave-invalid-properties clients/client1.yaml' \
   -c 'process name - --resource-method-version --overwrite' \
   -c 'validate openapi - --continue-on-error' \
   -c 'codegen openapi - --template common::resources/openapi/ --target-dir pkg/client1/ -o namespace=client1 -o clientName=Client1'
```

To target such registry you simply use: `--template REGISTRY_NAME::path/`

## Example of usage

This example shows openapi http server with two external openapi client dependencies:

```
schematools chain -vv \
  # 0. Register external repository with templates and fix it to tag
  -c 'registry add default https://codegen-templates/templates.git --tag v0.5.0' \
  # 1. Load local openapi specification from file and dereference
  -c 'process dereference spec/api.yaml --skip-root-internal-references --create-internal-references' \
  # 1. Convert allOf to structs
  -c 'process merge-all-of - --leave-invalid-properties' \
  # 1. Overwrite titles of schemas and operationIds of endpoints in openapi
  -c 'process name - --overwrite --resource-method-version' \
  # 1. Perform validation of our openapi specification - interrupt build on error
  -c 'validate openapi - ' \
  # 1. Create models and routers
  -c 'codegen openapi - --template default::rust-actix-server/ --format "rustfmt --edition 2018" --target-dir src/app/ -o name=ShippingApp'  \
  \
  # 2. Load remote openapi definition of external service
  -c 'process dereference https://schemas.com/openapi/orders/v0.1.0.json --skip-root-internal-references' \
  # 2. Convert allOf to structs
  -c 'process merge-all-of - --leave-invalid-properties' \
  # 2. Overwrite titles of schemas and operationIds of endpoints in openapi because it follow restful standards
  -c 'process name - --overwrite --resource-method-version' \
  # 2. Patch openapi specification because it has an error and we don't want to wait for a fix to be published by other project
  -c 'patch - apply specs/fixes/orders.yaml' \
  # 2. Validate openapi definition but continue on failure because it's an external client not owned by project
  -c 'validate openapi - --continue-on-error' \
  # 2. Create client
  -c "codegen openapi - --optional-and-nullable-as-models --template default::rust-reqwest-http/ --format 'rustfmt --edition 2018' \
    -o 'usedEndpoints=~[\"ordersListV3\",\"ordersCreateV3\"]' \
    --target-dir src/clients/ -o name=OrdersClient" \
  \
  # 3. Load remote openapi definition of external service
  -c 'process dereference https://schemas.com/openapi/users/v0.1.0.json --skip-root-internal-references' \
  # 3. Convert allOf to structs
  -c 'process merge-all-of - --leave-invalid-properties' \
  # 3. Overwrite titles of schemas and operationIds of endpoints in openapi because it follow restful standards
  -c 'process name - --overwrite --resource-method-version' \
  # 3. Validate openapi definition but continue on failure because it's an external client not owned by project
  -c 'validate openapi - --continue-on-error' \
  # 3. Create client
  -c "codegen openapi - --optional-and-nullable-as-models --template default::rust-reqwest-http/ --format 'rustfmt --edition 2018' \
    -o 'usedEndpoints=~[\"usersListV3\",\"usersCreateV3\"]' \
    --target-dir src/clients/ -o name=UsersClient"
```
