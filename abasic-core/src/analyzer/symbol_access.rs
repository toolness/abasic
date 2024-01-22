use std::collections::HashMap;

use crate::{
    program::{NumberedProgramLocation, ProgramLocation},
    symbol::Symbol,
};

pub enum SymbolAccessWarning {
    /// A symbol is read from, but never written to (i.e., defined).
    UndefinedSymbol,
    /// A symbol is written to (i.e., defined), but never read from.
    UnusedSymbol,
}

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

    pub fn get_warnings(&self) -> Vec<(SymbolAccessWarning, Symbol, NumberedProgramLocation)> {
        let mut warnings = vec![];
        for (symbol, locations) in &self.0 {
            if locations.reads.is_empty() && !locations.writes.is_empty() {
                for &location in &locations.writes {
                    warnings.push((SymbolAccessWarning::UnusedSymbol, symbol.clone(), location));
                }
            } else if locations.writes.is_empty() && !locations.reads.is_empty() {
                for &location in &locations.reads {
                    warnings.push((
                        SymbolAccessWarning::UndefinedSymbol,
                        symbol.clone(),
                        location,
                    ));
                }
            }
        }
        warnings
    }
}
