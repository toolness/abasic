use std::rc::Rc;

use crate::{program::ProgramLine, string_manager::StringManager};

#[derive(Debug)]
pub struct DataChunk {
    location: ProgramLine,
    data: Rc<Vec<DataElement>>,
}

impl DataChunk {
    pub fn new(location: ProgramLine, data: Rc<Vec<DataElement>>) -> Self {
        DataChunk { location, data }
    }
}

#[derive(Debug)]
pub struct DataIterator {
    chunks: Vec<DataChunk>,
    chunk_index: usize,
    chunk_item_index: usize,
}

impl DataIterator {
    pub fn new(chunks: Vec<DataChunk>) -> Self {
        Self {
            chunks,
            chunk_index: 0,
            chunk_item_index: 0,
        }
    }

    pub fn current_location(&self) -> Option<ProgramLine> {
        if let Some(chunk) = self.chunks.get(self.chunk_index) {
            Some(chunk.location)
        } else {
            None
        }
    }
}

impl Iterator for DataIterator {
    type Item = DataElement;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let Some(chunk) = self.chunks.get(self.chunk_index) else {
                return None;
            };
            let Some(element) = chunk.data.get(self.chunk_item_index) else {
                self.chunk_item_index = 0;
                self.chunk_index += 1;
                continue;
            };
            self.chunk_item_index += 1;
            return Some(element.clone());
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum DataElement {
    String(Rc<String>),
    Number(f64),
}

pub fn data_elements_to_string(elements: &Vec<DataElement>) -> String {
    elements
        .iter()
        .map(|element| match element {
            DataElement::String(string) => format!("\"{}\"", string),
            DataElement::Number(number) => number.to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Default, PartialEq)]
enum ParseState {
    #[default]
    Normal,
    InDoubleQuotedString,
}

#[derive(Default)]
struct DataParser<'a> {
    state: ParseState,
    elements: Vec<DataElement>,
    bytes_chomped: usize,
    current_element: String,
    is_finished: bool,
    string_manager: Option<&'a mut StringManager>,
}

impl<'a> DataParser<'a> {
    fn make_string_data_element(&mut self, string: String) -> DataElement {
        let rc_string = if let Some(manager) = &mut self.string_manager {
            manager.from_string(string)
        } else {
            Rc::new(string)
        };
        DataElement::String(rc_string)
    }

    fn push_current_element(&mut self) {
        let mut string_value = std::mem::take(&mut self.current_element);

        let element = if self.state != ParseState::InDoubleQuotedString {
            string_value = string_value.trim().to_string();
            if let Ok(number) = string_value.parse::<f64>() {
                DataElement::Number(number)
            } else {
                self.make_string_data_element(string_value)
            }
        } else {
            self.make_string_data_element(string_value)
        };

        self.elements.push(element);
    }

    fn parse_char(&mut self, char: char) {
        match self.state {
            ParseState::Normal => match char {
                ':' => {
                    self.finish();
                }
                ',' => {
                    if !self.current_element.trim().is_empty() {
                        self.push_current_element();
                    }
                }
                '"' => {
                    if self.current_element.trim().is_empty() {
                        self.current_element.clear();
                        self.state = ParseState::InDoubleQuotedString;
                    } else {
                        self.current_element.push(char);
                    }
                }
                _ => {
                    self.current_element.push(char);
                }
            },
            ParseState::InDoubleQuotedString => match char {
                '"' => {
                    self.push_current_element();
                    self.state = ParseState::Normal;
                }
                _ => {
                    self.current_element.push(char);
                }
            },
        }
        if !self.is_finished {
            self.bytes_chomped += char.len_utf8();
        }
    }

    fn finish(&mut self) {
        if self.is_finished {
            return;
        }

        if self.current_element.len() > 0 {
            self.push_current_element();
        } else if self.elements.len() == 0 {
            self.push_current_element();
        }

        self.is_finished = true;
    }
}

/// This is super weird because DATA statements are super weird.
///
/// It doesn't conform completely to Applesoft BASIC but it gets us
/// most of the way there.
///
/// Note that this will never return an empty `Vec`: even if the string
/// is empty, it will still return a single-element `Vec` with an empty
/// string in it.
pub fn parse_data_until_colon(
    value: &str,
    string_manager: Option<&mut StringManager>,
) -> (Vec<DataElement>, usize) {
    let mut parser = DataParser::default();

    parser.string_manager = string_manager;

    for char in value.chars() {
        parser.parse_char(char);
        if parser.is_finished {
            break;
        }
    }

    parser.finish();

    (parser.elements, parser.bytes_chomped)
}

#[cfg(test)]
pub mod test_util {
    use std::rc::Rc;

    use super::DataElement;

    pub fn string(value: &'static str) -> DataElement {
        DataElement::String(Rc::new(value.to_string()))
    }

    pub fn number(value: f64) -> DataElement {
        DataElement::Number(value)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::{
        data::{parse_data_until_colon, DataChunk},
        program::ProgramLine,
    };

    use super::{
        test_util::{number, string},
        DataElement, DataIterator,
    };

    fn assert_parse_all_data(value: &'static str, expect: &[DataElement]) {
        assert_eq!(
            parse_data_until_colon(value, None),
            (Vec::from(expect), value.len()),
            "Parsing '{}'",
            value
        );
    }

    fn assert_parse_partial_data(
        value: &'static str,
        expect_elements: &[DataElement],
        expect_bytes_chomped: usize,
        expect_unchomped_str: &'static str,
    ) {
        assert_eq!(
            parse_data_until_colon(value, None),
            (Vec::from(expect_elements), expect_bytes_chomped),
            "Parsing '{}' (expecting partial data)",
            value
        );
        assert_eq!(
            &value[expect_bytes_chomped..],
            expect_unchomped_str,
            "Parsing '{}' (comparing unchomped str)",
            value
        );
    }

    #[test]
    fn empty_data_iterator_works() {
        let mut iterator = DataIterator::new(vec![]);
        assert_eq!(iterator.next(), None);
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn non_empty_data_iterator_works() {
        let mut iterator = DataIterator::new(vec![
            DataChunk::new(
                ProgramLine::Line(10),
                Rc::new(vec![string("hi"), number(1.0)]),
            ),
            DataChunk::new(ProgramLine::Line(20), Rc::new(vec![string("boop")])),
        ]);
        assert_eq!(iterator.current_location(), Some(ProgramLine::Line(10)));
        assert_eq!(iterator.next(), Some(string("hi")));
        assert_eq!(iterator.current_location(), Some(ProgramLine::Line(10)));
        assert_eq!(iterator.next(), Some(number(1.0)));
        assert_eq!(iterator.current_location(), Some(ProgramLine::Line(10)));
        assert_eq!(iterator.next(), Some(string("boop")));
        assert_eq!(iterator.current_location(), Some(ProgramLine::Line(20)));
        assert_eq!(iterator.next(), None);
        assert_eq!(iterator.current_location(), None);
    }

    #[test]
    fn parsing_empty_string_works() {
        assert_parse_all_data("", &[string("")]);
        assert_parse_all_data("    ", &[string("")]);
    }

    #[test]
    fn parsing_one_unquoted_string_works() {
        assert_parse_all_data(" boop", &[string("boop")]);
        assert_parse_all_data(" 123boop", &[string("123boop")]);
    }

    #[test]
    fn parsing_one_quoted_string_works() {
        assert_parse_all_data(" \"boop\"", &[string("boop")]);
    }

    #[test]
    fn parsing_multiple_unquoted_strings_works() {
        assert_parse_all_data(
            " foo,  bar,  baz",
            &[string("foo"), string("bar"), string("baz")],
        );
    }

    #[test]
    fn quoted_strings_are_not_trimmed() {
        assert_parse_all_data("\" boop  \"", &[string(" boop  ")]);
    }

    #[test]
    fn unicode_works() {
        assert_parse_all_data("\"hi 😊\"", &[string("hi 😊")]);
    }

    #[test]
    fn parsing_single_number_works() {
        assert_parse_all_data("1", &[number(1.0)]);
        assert_parse_all_data(" 1 ", &[number(1.0)]);
        assert_parse_all_data(" 1.0 ", &[number(1.0)]);
        assert_parse_all_data(" .1 ", &[number(0.1)]);
    }

    #[test]
    fn parsing_multiple_numbers_works() {
        assert_parse_all_data("1, 2, 3", &[number(1.0), number(2.0), number(3.0)]);
    }

    #[test]
    fn parsing_quoted_then_unquoted_string_works() {
        assert_parse_all_data("\"  foo\",bar", &[string("  foo"), string("bar")]);
    }

    #[test]
    fn parsing_heterogenous_types_works() {
        assert_parse_all_data(
            "foo, \"  bar\",  baz , 3",
            &[string("foo"), string("  bar"), string("baz"), number(3.0)],
        );
    }

    #[test]
    fn parsing_unquoted_string_with_quotes_in_it_works() {
        // Applesoft BASIC supports this.
        assert_parse_all_data("hello \"there\"", &[string("hello \"there\"")]);
    }

    #[test]
    fn parsing_unquoted_string_with_quotes_in_does_not_work_when_starting_with_quoted_string() {
        // This is weird but might as well document it.
        //
        // Note that Applesoft BASIC will actually reject this with a syntax error, e.g.:
        //
        //   10 DATA "hello" there
        //   20 READ A$
        //
        // Running this will print "SYNTAX ERROR IN 10".
        assert_parse_all_data("\"hello\" there", &[string("hello"), string("there")]);
    }

    #[test]
    fn parsing_does_not_stop_at_colon_in_quoted_strings() {
        assert_parse_all_data("\"foo:::\"", &[string("foo:::")]);
    }

    #[test]
    fn parsing_stops_at_colon() {
        assert_parse_partial_data(" foo:blah", &[string("foo")], 4, ":blah");
        assert_parse_partial_data(" \"foo:\":blah", &[string("foo:")], 7, ":blah");
        assert_parse_partial_data(" foo😊:blah", &[string("foo😊")], 8, ":blah");
        assert_parse_partial_data(" \"foo😊\":blah", &[string("foo😊")], 10, ":blah");
    }
}
