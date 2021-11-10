//! jsonref dereferences JSONSchema `$ref` attributes and creates a new dereferenced schema.
//!
//! Dereferencing is normally done by a JSONSchema validator in the process of validation, but
//! it is sometimes useful to do this independent of the validator for tasks like:
//! 
//! * Analysing a schema programatically to see what field there are.
//! * Programatically modifying a schema.
//! * Passing to tools that create fake JSON data from the schema.
//! * Passing the schema to form generation tools.
//!
//! This crate is intended to do this for you.
//!
//! Example:
//! ```
//! use serde_json::json;
//! use jsonref::JsonRef;
//!
//! let mut simple_example = json!(
//!           {"properties": {"prop1": {"title": "name"},
//!                           "prop2": {"$ref": "#/properties/prop1"}}
//!           }
//!        );
//!
//! let mut jsonref = JsonRef::new();
//! let dereffed = jsonref.deref_value(simple_example).unwrap();
//!
//! let dereffed_expected = json!(
//!     {"properties": {"prop1": {"title": "name"},
//!      "prop2": {"title": "name", "__reference__": {}}}}
//! );
//! assert_eq!(dereffed, dereffed_expected)
//! ```

use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::mem;
use url::Url;
use std::path::PathBuf;
use std::error::Error;

/// main struct that holds configurationo for a JSONScheam drefferencing
#[derive(Debug)]
pub struct JsonRef {
    schema_cache: HashMap<String, Value>,
}

impl JsonRef {
    pub fn new() -> JsonRef {
        return JsonRef {
            schema_cache: HashMap::new(),
        };
    }

    pub fn deref_value(&mut self, mut value: Value) -> Result<Value, Box<dyn Error>> {
        let anon_file_url = format!(
            "file://{}/anon.json",
            env::current_dir()?.to_string_lossy()
        );
        self.schema_cache.insert(anon_file_url.clone(), value.clone());

        self.deref(&mut value, anon_file_url, &vec![])?;
        Ok(value)
    }

    pub fn deref_url(&mut self, url: String) -> Result<Value, Box<dyn Error>> {
        let mut value: Value = reqwest::blocking::get(&url)?.json()?;

        self.schema_cache.insert(url.clone(), value.clone());
        self.deref(&mut value, url, &vec![])?;
        Ok(value)
    }

    pub fn deref_file(&mut self, file_path: String) -> Result<Value, Box<dyn Error>> {

        let file = fs::File::open(&file_path)?;
        let mut value: Value = serde_json::from_reader(file)?;
        let path = PathBuf::from(file_path);
        let absolute_path  = fs::canonicalize(path)?;
        let url = format!("file://{}", absolute_path.to_string_lossy());

        self.schema_cache.insert(url.clone(), value.clone());
        self.deref(&mut value, url, &vec![])?;
        Ok(value)
    }

    fn deref(&mut self, value: &mut Value, id: String, used_refs: &Vec<String>) -> Result<(), Box<dyn Error>> {
        let mut new_id = id;
        if let Some(id_value) = value.get("$id") {
            if let Some(id_string) = id_value.as_str() {
                new_id = id_string.to_string()
            }
        }

        if let Some(obj) = value.as_object_mut() {
            if let Some(ref_value) = obj.remove("$ref") {
                if let Some(ref_string) = ref_value.as_str() {
                    let id_url = Url::parse(&new_id)?; //handle error
                    let ref_url = id_url.join(ref_string)?;

                    let mut ref_url_no_fragment = ref_url.clone();
                    ref_url_no_fragment.set_fragment(None);
                    let ref_no_fragment = ref_url_no_fragment.to_string();

                    let mut schema = match self.schema_cache.get(&ref_no_fragment) {
                        Some(cached_schema) => cached_schema.clone(),
                        None => {
                            if ref_no_fragment.starts_with("http") {
                                reqwest::blocking::get(&ref_no_fragment)?.json()?
                            } else if ref_no_fragment.starts_with("file") {
                                let file = fs::File::open(ref_url_no_fragment.path())?;
                                serde_json::from_reader(file)?
                            } else {
                                panic!("need url to be a file or a http based url")
                            }
                        }
                    };

                    if !self.schema_cache.contains_key(&ref_no_fragment) {
                        self.schema_cache
                            .insert(ref_no_fragment.clone(), schema.clone());
                    }

                    if let Some(ref_fragment) = ref_url.fragment() {
                        schema = schema.pointer(ref_fragment) .unwrap().to_owned();
                        //handle this better
                    }
                    let ref_url_string = ref_url.to_string();
                    if used_refs.contains(&ref_url_string) {
                        return Ok(())
                    }

                    let mut new_used_refs = used_refs.clone();
                    new_used_refs.push(ref_url_string);


                    self.deref(&mut schema, ref_no_fragment, &new_used_refs)?;

                    let old_value = mem::replace(value, schema);
                    if let Some(new_obj) = value.as_object_mut() {
                        new_obj.insert("__reference__".to_string(), old_value);
                    }
                }
            }
        }

        if let Some(obj) = value.as_object_mut() {
            for obj_value in obj.values_mut() {
                self.deref(obj_value, new_id.clone(), used_refs)?
            }
        }
    Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::JsonRef;
    use std::fs;
    use serde_json::{json, Value};

    #[test]
    fn json_no_refs() {
        let no_ref_example = json!({"properties": {"prop1": {"title": "proptitle"}}});

        let mut jsonref = JsonRef::new();

        let mut input = no_ref_example.clone();

        input = jsonref.deref_value(input).unwrap();

        assert_eq!(input, no_ref_example)
    }

    #[test]
    fn json_simple() {
        let mut simple_refs_example = json!(
            {"properties": {"prop1": {"title": "name"},
                            "prop2": {"$ref": "#/properties/prop1"}}
            }
        );

        let simple_refs_expected = json!(
            {"properties": {"prop1": {"title": "name"},
                            "prop2": {"title": "name", "__reference__": {}}}
            }
        );

        let mut jsonref = JsonRef::new();
        simple_refs_example = jsonref.deref_value(simple_refs_example).unwrap();

        assert_eq!(simple_refs_example, simple_refs_expected)
    }

    #[test]
    fn json_simple_with_extra_data() {
        let mut simple_refs_example = json!(
            {"properties": {"prop1": {"title": "name"},
                            "prop2": {"$ref": "#/properties/prop1", "title": "old_title"}}
            }
        );

        let simple_refs_expected = json!(
            {"properties": {"prop1": {"title": "name"},
                            "prop2": {"title": "name", "__reference__": {"title": "old_title"}}}
            }
        );

        let mut jsonref = JsonRef::new();
        simple_refs_example = jsonref.deref_value(simple_refs_example).unwrap();

        assert_eq!(simple_refs_example, simple_refs_expected)
    }

    #[test]
    fn simple_from_url() {
        let mut simple_refs_example = json!(
            {"properties": {"prop1": {"title": "name"},
                            "prop2": {"$ref": "https://gist.githubusercontent.com/kindly/35a631d33792413ed8e34548abaa9d61/raw/b43dc7a76cc2a04fde2a2087f0eb389099b952fb/test.json", "title": "old_title"}}
            }
        );

        let simple_refs_expected = json!(
            {"properties": {"prop1": {"title": "name"},
                            "prop2": {"title": "title from url", "__reference__": {"title": "old_title"}}}
            }
        );

        let mut jsonref = JsonRef::new();
        simple_refs_example = jsonref.deref_value(simple_refs_example).unwrap();

        assert_eq!(simple_refs_example, simple_refs_expected)
    }

    #[test]
    fn nested_with_ref_from_url() {
        let mut simple_refs_example = json!(
            {"properties": {"prop1": {"title": "name"},
                            "prop2": {"$ref": "https://gist.githubusercontent.com/kindly/35a631d33792413ed8e34548abaa9d61/raw/0a691c035251f742e8710f71ba92ead307823385/test_nested.json"}}
            }
        );

        let simple_refs_expected = json!(
            {"properties": {"prop1": {"title": "name"},
                            "prop2": {"__reference__": {},
                                      "title": "title from url",
                                      "properties": {"prop1": {"title": "sub property title in url"},
                                                     "prop2": {"__reference__": {}, "title": "sub property title in url"}}
                            }}
            }
        );

        let mut jsonref = JsonRef::new();
        simple_refs_example = jsonref.deref_value(simple_refs_example).unwrap();

        assert_eq!(simple_refs_example, simple_refs_expected)
    }

    #[test]
    fn nested_ref_from_local_file() {

        let mut jsonref = JsonRef::new();
        let file_example = jsonref.deref_file("fixtures/nested_relative/base.json".to_string()).unwrap();

        let file = fs::File::open("fixtures/nested_relative/expected.json").unwrap();
        let file_expected: Value = serde_json::from_reader(file).unwrap();

        println!("{}", serde_json::to_string_pretty(&file_example).unwrap());

        assert_eq!(file_example, file_expected)
    }

    #[test]
    fn nested_ref_from_url() {

        let mut jsonref = JsonRef::new();
        let file_example = jsonref.deref_url("https://gist.githubusercontent.com/kindly/91e09f88ced65aaca1a15d85a56a28f9/raw/52f8477435cff0b73c54aacc70926c101ce6c685/base.json".to_string()).unwrap();

        let file = fs::File::open("fixtures/nested_relative/expected.json").unwrap();
        let file_expected: Value = serde_json::from_reader(file).unwrap();

        println!("{}", serde_json::to_string_pretty(&file_example).unwrap());

        assert_eq!(file_example, file_expected)
    }

}
