use std::{collections::HashSet, rc::Rc};

#[derive(Default)]
pub struct StringInterner {
    strings: HashSet<Rc<String>>,
}

impl StringInterner {
    pub fn get(&mut self, string: String) -> Rc<String> {
        // This probably isn't really worth the trouble since the only strings
        // we're likely to repeat are symbol names like variables and functions,
        // and those are probably going to be pretty short, but still, it's a
        // fun exercise, and there's something inherently satisfying about it.
        let result = self.strings.get(&string);

        match result {
            Some(interned_string) => interned_string.clone(),
            None => {
                let interned_string = Rc::new(string);
                self.strings.insert(interned_string.clone());
                interned_string
            }
        }
    }
}
