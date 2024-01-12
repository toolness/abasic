use std::{collections::HashSet, rc::Rc};

#[derive(Default)]
pub struct StringManager {
    strings: HashSet<Rc<String>>,
    total_bytes: usize,
}

impl StringManager {
    pub fn from_str<T: AsRef<str>>(&mut self, value: T) -> Rc<String> {
        for string in &self.strings {
            if string.as_str() == value.as_ref() {
                return string.clone();
            }
        }
        self.add(value.as_ref().to_string())
    }

    pub fn from_string(&mut self, value: String) -> Rc<String> {
        if let Some(string) = self.strings.get(&value) {
            string.clone()
        } else {
            self.add(value)
        }
    }

    fn add(&mut self, value: String) -> Rc<String> {
        let new_entry = Rc::new(value);
        self.strings.insert(new_entry.clone());
        self.total_bytes += new_entry.len();
        new_entry
    }

    pub fn gc(&mut self) {
        let mut weak_refs = self
            .strings
            .drain()
            .map(|string| Rc::downgrade(&string))
            .collect::<Vec<_>>();

        self.strings = weak_refs
            .drain(..)
            .filter_map(|weak| weak.upgrade())
            .collect::<HashSet<_>>();

        self.total_bytes = self.strings.iter().map(|string| string.len()).sum();
    }

    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::StringManager;

    #[test]
    fn it_works() {
        let mut manager = StringManager::default();
        let foo_string = String::from("foo");
        let foo_str = "foo";
        assert_ne!(foo_string.as_str() as *const str, foo_str as *const str);
        let a = manager.from_str(foo_str);
        let b = manager.from_string(foo_string);
        assert_eq!(a, b);
        assert_eq!(a.as_str() as *const str, b.as_str() as *const str);
        assert_eq!(manager.total_bytes(), 3);
        manager.gc();
        assert_eq!(manager.total_bytes(), 3);
        drop(a);
        drop(b);
        assert_eq!(manager.total_bytes(), 3);
        manager.gc();
        assert_eq!(manager.total_bytes(), 0);
    }
}
