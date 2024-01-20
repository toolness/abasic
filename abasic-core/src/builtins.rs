use crate::symbol::Symbol;

pub enum Builtin {
    Abs,
    Int,
    Rnd,
}

impl Builtin {
    pub fn try_from(value: &Symbol) -> Option<Builtin> {
        Some(match value.as_str() {
            "ABS" => Builtin::Abs,
            "INT" => Builtin::Int,
            "RND" => Builtin::Rnd,
            _ => return None,
        })
    }
}
