use jzon::{JsonValue, object, array};

pub struct Gltf {
    pub root: jzon::JsonValue,
}

pub fn pushToField(obj: &mut JsonValue, field: &str, val: JsonValue) -> usize {
    if !obj.has_key(field) {
        obj.insert(field, array! [ ]).unwrap();
    }
    let handle = obj[field].len();
    obj[field].push(val).unwrap();
    handle
}

impl Gltf {
    pub fn new() -> Gltf {
        Gltf {
            root: object! { }
        }
    }
    pub fn add(&mut self, objCollection: &str, val: JsonValue) -> usize {
        pushToField(&mut self.root, objCollection, val)
    }
}