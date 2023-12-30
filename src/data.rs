use std::rc::Rc;

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
struct DataParser {
    state: ParseState,
    elements: Vec<DataElement>,
    bytes_chomped: usize,
    current_element: String,
    is_finished: bool,
}

impl DataParser {
    fn push_current_element(&mut self) {
        let mut string_value = std::mem::take(&mut self.current_element);

        let element = if self.state != ParseState::InDoubleQuotedString {
            string_value = string_value.trim().to_string();
            if let Ok(number) = string_value.parse::<f64>() {
                DataElement::Number(number)
            } else {
                DataElement::String(Rc::new(string_value))
            }
        } else {
            DataElement::String(Rc::new(string_value))
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
pub fn parse_data_until_colon(value: &str) -> (Vec<DataElement>, usize) {
    let mut parser = DataParser::default();

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
mod tests {
    use std::rc::Rc;

    use crate::data::parse_data_until_colon;

    use super::DataElement;

    fn string(value: &'static str) -> DataElement {
        DataElement::String(Rc::new(value.to_string()))
    }

    fn number(value: f64) -> DataElement {
        DataElement::Number(value)
    }

    fn assert_parse_all_data(value: &'static str, expect: &[DataElement]) {
        assert_eq!(
            parse_data_until_colon(value),
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
            parse_data_until_colon(value),
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
        // This is weird but might as well document it...
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
