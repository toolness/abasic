use std::collections::HashMap;

use crate::{
    program::{NumberedProgramLocation, ProgramLocation},
    symbol::Symbol,
};

pub enum SymbolAccess {
    Read,
    Write,
}

pub struct SymbolAccessLocation(SymbolAccess, NumberedProgramLocation);

#[derive(Default)]
pub struct SymbolAccessMap(HashMap<Symbol, Vec<SymbolAccessLocation>>);

impl SymbolAccessMap {
    pub fn log_access(
        &mut self,
        symbol: &Symbol,
        location: &ProgramLocation,
        access: SymbolAccess,
    ) {
        let entry = self.0.entry(symbol.clone()).or_default();
        entry.push(SymbolAccessLocation(
            access,
            // We're analyzing code, so we should always be passed in a
            // numbered program location.
            (*location).try_into().unwrap(),
        ));
    }
}
