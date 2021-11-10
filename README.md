jsonref dereferences JSONSchema `$ref` attributes and creates a new dereferenced schema.

Dereferencing is normally done by a JSONSchema validator in the process of validation, but
it is sometimes useful to do this independent of the validator for tasks like:

* Analysing a schema programatically to see what field there are.
* Programatically modifying a schema.
* Passing to tools that create fake JSON data from the schema.
* Passing the schema to form generation tools.

This crate is intended to do this for you.

Example:

```rust
use serde_json::json;
use jsonref::JsonRef;

let mut simple_example = json!(
          {"properties": {"prop1": {"title": "name"},
                          "prop2": {"$ref": "#/properties/prop1"}}
          }
       );

let mut jsonref = JsonRef::new();
let dereffed = jsonref.deref_value(simple_example).unwrap();

let dereffed_expected = json!(
    {"properties": {"prop1": {"title": "name"},
     "prop2": {"title": "name", "__reference__": {}}}}
);
assert_eq!(dereffed, dereffed_expected)
```
