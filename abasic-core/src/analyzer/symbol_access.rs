use std::collections::HashMap;

use crate::{
    program::{NumberedProgramLocation, ProgramLocation},
    symbol::Symbol,
};

pub enum SymbolAccess {
    Read,
    Write,
}

#[derive(Default)]
struct SymbolAccessLocations {
    writes: Vec<NumberedProgramLocation>,
    reads: Vec<NumberedProgramLocation>,
}

#[derive(Default)]
pub struct SymbolAccessMap(HashMap<Symbol, SymbolAccessLocations>);

impl SymbolAccessMap {
    pub fn log_access(
        &mut self,
        symbol: &Symbol,
        location: &ProgramLocation,
        access: SymbolAccess,
    ) {
        let entry = self.0.entry(symbol.clone()).or_default();
        let target = match access {
            SymbolAccess::Read => &mut entry.reads,
            SymbolAccess::Write => &mut entry.writes,
        };

        // We're analyzing code, so we should always be passed in a
        // numbered program location.
        target.push((*location).try_into().unwrap());
    }
}
