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
//!
//! jsonref.deref_value(&mut simple_example).unwrap();
//!
//! let dereffed_expected = json!(
//!     {"properties": 
//!         {"prop1": {"title": "name"},
//!          "prop2": {"title": "name"}}
//!     }
//! );
//! assert_eq!(simple_example, dereffed_expected)
//! ```
//!
//! **Note**:  If the JSONSchema has recursive `$ref` only the first recursion will happen.
//! This is to stop an infinate loop.

use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::mem;
use std::path::PathBuf;
use url::Url;

/// Main struct that holds configuration for a JSONScheama derefferencing.
///
/// Instantiate with
/// ```
/// use jsonref::JsonRef;
/// let jsonref = JsonRef::new();
/// ```
///
/// Configuration is done through the `set_` methods on the struct.
#[derive(Debug)]
pub struct JsonRef {
    schema_cache: HashMap<String, Value>,
    reference_key: Option<String>,
}

impl JsonRef {
    /// Create a new instance of JsonRef.
    pub fn new() -> JsonRef {
        return JsonRef {
            schema_cache: HashMap::new(),
            reference_key: None,
        };
    }

    /// Set a key to store the data that the `$ref` replaced. 
    ///
    /// This example uses `__reference__` as the key.
    /// 
    /// ```
    /// # use jsonref::JsonRef;
    /// # let jsonref = JsonRef::new();
    /// use serde_json::json;
    ///
    /// let mut input  = json!(
    ///     {"properties": {"prop1": {"title": "name"},
    ///                     "prop2": {"$ref": "#/properties/prop1", "title": "old_title"}}
    ///     }
    /// );
    ///                                                                                          
    /// let expected = json!(
    ///     {"properties": {"prop1": {"title": "name"},
    ///                     "prop2": {"title": "name", "__reference__": {"title": "old_title"}}}
    ///     }
    /// );
    ///                                                                                          
    /// let mut jsonref = JsonRef::new();
    ///
    /// jsonref.set_reference_key("__reference__");
    ///
    /// jsonref.deref_value(&mut input).unwrap();
    ///                                                                                          
    /// assert_eq!(input, expected)
    /// ```

    pub fn set_reference_key(&mut self, reference_key: &str) {
        self.reference_key = Some(reference_key.to_owned());
    }

    /// deref a serde_json value directly. Uses the current working directory for any relative
    /// refs.
    pub fn deref_value(&mut self, value: &mut Value) -> Result<(), Box<dyn Error>> {
        let anon_file_url = format!("file://{}/anon.json", env::current_dir()?.to_string_lossy());
        self.schema_cache
            .insert(anon_file_url.clone(), value.clone());

        self.deref(value, anon_file_url, &vec![])?;
        Ok(())
    }

    /// deref from a URL:
    ///
    /// ```
    /// # use jsonref::JsonRef;
    /// # let jsonref = JsonRef::new();
    /// # use serde_json::Value;
    /// # use std::fs;
    /// let mut jsonref = JsonRef::new();
    /// # jsonref.set_reference_key("__reference__");
    /// let input_url = jsonref.deref_url("https://gist.githubusercontent.com/kindly/91e09f88ced65aaca1a15d85a56a28f9/raw/52f8477435cff0b73c54aacc70926c101ce6c685/base.json").unwrap();
    /// # let file = fs::File::open("fixtures/nested_relative/expected.json").unwrap();
    /// # let file_expected: Value = serde_json::from_reader(file).unwrap();
    /// # assert_eq!(input_url, file_expected)
    /// ```
    pub fn deref_url(&mut self, url: &str) -> Result<Value, Box<dyn Error>> {
        let mut value: Value = reqwest::blocking::get(url)?.json()?;

        self.schema_cache.insert(url.to_string(), value.clone());
        self.deref(&mut value, url.to_string(), &vec![])?;
        Ok(value)
    }

    /// deref from a File:
    ///
    /// ```
    /// # use jsonref::JsonRef;
    /// # let jsonref = JsonRef::new();
    /// # use serde_json::Value;
    /// # use std::fs;
    ///
    /// let mut jsonref = JsonRef::new();
    /// # jsonref.set_reference_key("__reference__");
    /// let file_example = jsonref
    ///     .deref_file("fixtures/nested_relative/base.json")
    ///     .unwrap();
    /// # let file = fs::File::open("fixtures/nested_relative/expected.json").unwrap();
    /// # let file_expected: Value = serde_json::from_reader(file).unwrap();
    /// # assert_eq!(file_example, file_expected)
    /// ```
    pub fn deref_file(&mut self, file_path: &str) -> Result<Value, Box<dyn Error>> {
        let file = fs::File::open(file_path)?;
        let mut value: Value = serde_json::from_reader(file)?;
        let path = PathBuf::from(file_path);
        let absolute_path = fs::canonicalize(path)?;
        let url = format!("file://{}", absolute_path.to_string_lossy());

        self.schema_cache.insert(url.clone(), value.clone());
        self.deref(&mut value, url, &vec![])?;
        Ok(value)
    }

    fn deref(
        &mut self,
        value: &mut Value,
        id: String,
        used_refs: &Vec<String>,
    ) -> Result<(), Box<dyn Error>> {
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

                    let ref_url_string = ref_url.to_string();
                    if let Some(ref_fragment) = ref_url.fragment() {
                        schema = schema.pointer(ref_fragment).ok_or(
                            format!("ref `{}` can not be resolved as pointer `{}` can not be found in the schema", ref_string, ref_fragment))?.clone();
                    }
                    if used_refs.contains(&ref_url_string) {
                        return Ok(());
                    }

                    let mut new_used_refs = used_refs.clone();
                    new_used_refs.push(ref_url_string);

                    self.deref(&mut schema, ref_no_fragment, &new_used_refs)?;
                    let old_value = mem::replace(value, schema);

                    if let Some(reference_key) = &self.reference_key {
                        if let Some(new_obj) = value.as_object_mut() {
                            new_obj.insert(reference_key.clone(), old_value);
                        }
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
    use serde_json::{json, Value};
    use std::fs;

    #[test]
    fn json_no_refs() {
        let no_ref_example = json!({"properties": {"prop1": {"title": "proptitle"}}});

        let mut jsonref = JsonRef::new();

        let mut input = no_ref_example.clone();

        jsonref.deref_value(&mut input).unwrap();

        assert_eq!(input, no_ref_example)
    }

    #[test]
    fn json_with_recursion() {
        let mut simple_refs_example = json!(
            {"properties": {"prop1": {"$ref": "#"}}}
        );

        let simple_refs_expected = json!(
            {"properties": {"prop1": {"properties": {"prop1": {}}}}
            }
        );

        let mut jsonref = JsonRef::new();
        jsonref.deref_value(&mut simple_refs_example).unwrap();
        jsonref.set_reference_key("__reference__");

        println!(
            "{}",
            serde_json::to_string_pretty(&simple_refs_example).unwrap()
        );

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
        jsonref.set_reference_key("__reference__");
        jsonref.deref_value(&mut simple_refs_example).unwrap();

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
        jsonref.set_reference_key("__reference__");
        jsonref.deref_value(&mut simple_refs_example).unwrap();

        assert_eq!(simple_refs_example, simple_refs_expected)
    }

    #[test]
    fn nested_ref_from_local_file() {
        let mut jsonref = JsonRef::new();
        jsonref.set_reference_key("__reference__");
        let file_example = jsonref
            .deref_file("fixtures/nested_relative/base.json")
            .unwrap();

        let file = fs::File::open("fixtures/nested_relative/expected.json").unwrap();
        let file_expected: Value = serde_json::from_reader(file).unwrap();

        println!("{}", serde_json::to_string_pretty(&file_example).unwrap());

        assert_eq!(file_example, file_expected)
    }

}
