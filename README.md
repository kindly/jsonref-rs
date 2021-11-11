jsonref dereferences JSONSchema `$ref` attributes and creates a new dereferenced schema.

Dereferencing is normally done by a JSONSchema validator in the process of validation, but
it is sometimes useful to do this independent of the validator for tasks like:

* Analysing a schema programatically to see what field there are.
* Programatically modifying a schema.
* Passing to tools that create fake JSON data from the schema.
* Passing the schema to form generation tools.


Example:
```
use serde_json::json;
use jsonref::JsonRef;

let mut simple_example = json!(
          {"properties": {"prop1": {"title": "name"},
                          "prop2": {"$ref": "#/properties/prop1"}}
          }
       );

let mut jsonref = JsonRef::new();

jsonref.deref_value(&mut simple_example).unwrap();

let dereffed_expected = json!(
    {"properties": 
        {"prop1": {"title": "name"},
         "prop2": {"title": "name"}}
    }
);
assert_eq!(simple_example, dereffed_expected)
```

**Note**:  If the JSONSchema has recursive `$ref` only the first recursion will happen.
This is to stop an infinate loop.
