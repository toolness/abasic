use core::fmt::Debug;
use std::collections::{BTreeSet, HashMap};

use crate::{
    data::{DataChunk, DataIterator},
    program::NumberedProgramLocation,
    tokenizer::Token,
};

#[derive(Default)]
pub struct ProgramLines {
    numbered_lines: HashMap<u64, Vec<Token>>,
    /// According to Wikipedia, Applesoft BASIC stored lines as a linked list,
    /// which meant that GOSUB/GOTO took linear time. This was likely due to
    /// memory constraints. We don't have such constraints, so we'll use a
    /// BTreeSet for faster lookup.
    sorted_line_numbers: BTreeSet<u64>,
}

impl Debug for ProgramLines {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut lines = f.debug_struct("ProgramLines");
        for line in &self.sorted_line_numbers {
            lines.field(
                line.to_string().as_str(),
                self.numbered_lines.get(line).unwrap(),
            );
        }
        lines.finish()
    }
}

impl ProgramLines {
    pub fn first(&self) -> Option<u64> {
        self.sorted_line_numbers.first().copied()
    }

    pub fn after(&self, line: u64) -> Option<u64> {
        self.sorted_line_numbers.range(line + 1..).next().copied()
    }

    pub fn has(&self, line_number: u64) -> bool {
        self.numbered_lines.contains_key(&line_number)
    }

    pub fn data_iterator(&self) -> DataIterator {
        let mut chunks = vec![];
        for &line in self.sorted_line_numbers.iter() {
            for (token_index, token) in self.numbered_lines.get(&line).unwrap().iter().enumerate() {
                if let Token::Data(data) = token {
                    chunks.push(DataChunk::new(
                        NumberedProgramLocation::new(line, token_index).into(),
                        data.clone(),
                    ));
                }
            }
        }
        DataIterator::new(chunks)
    }

    pub fn get(&self, line_number: u64) -> Option<&Vec<Token>> {
        self.numbered_lines.get(&line_number)
    }

    pub fn set(&mut self, line_number: u64, tokens: Vec<Token>) {
        if tokens.is_empty() {
            self.sorted_line_numbers.remove(&line_number);
            self.numbered_lines.remove(&line_number);
        } else {
            self.sorted_line_numbers.insert(line_number);
            self.numbered_lines.insert(line_number, tokens);
        }
    }

    pub fn list(&self) -> Vec<String> {
        let mut lines: Vec<String> = Vec::with_capacity(self.numbered_lines.len());

        for line_number in &self.sorted_line_numbers {
            let line = self
                .numbered_lines
                .get(line_number)
                .unwrap()
                .iter()
                .map(|token| token.to_string())
                .collect::<Vec<String>>()
                .join(" ");

            let line_source = format!("{} {}\n", line_number, line);
            lines.push(line_source);
        }

        lines
    }
}
