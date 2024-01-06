/// Attempt to parse the BASIC line number at the beginning of the given
/// string, skipping any leading whitespace.
///
/// If a line number is found, returns a tuple containing the parsed number
/// and the index of the character immediately after the line number's
/// final digit.
pub fn parse_line_number<T: AsRef<str>>(value: T) -> Option<(u64, usize)> {
    let mut number_endpoints: Option<(usize, usize)> = None;

    for (index, char) in value.as_ref().char_indices() {
        match number_endpoints {
            Some((start, _)) => {
                if char.is_ascii_digit() {
                    number_endpoints = Some((start, index + 1));
                } else {
                    break;
                }
            }
            None => {
                if char.is_ascii_digit() {
                    number_endpoints = Some((index, index + 1));
                } else if char.is_ascii_whitespace() {
                    continue;
                } else {
                    return None;
                }
            }
        }
    }

    let Some((start, end)) = number_endpoints else {
        return None;
    };
    let Ok(number) = value.as_ref()[start..end].parse::<u64>() else {
        return None;
    };
    Some((number, end))
}

#[cfg(test)]
mod tests {
    use crate::line_number_parser::parse_line_number;

    #[test]
    fn it_parses_line_numbers() {
        assert_eq!(parse_line_number("15 PRINT X"), Some((15, 2)));
        assert_eq!(parse_line_number(" 15 PRINT X"), Some((15, 3)));
        assert_eq!(parse_line_number("015 PRINT X"), Some((15, 3)));
        assert_eq!(parse_line_number("15 PRINT X"), Some((15, 2)));
    }

    #[test]
    fn it_returns_none_for_unnumbered_lines() {
        assert_eq!(parse_line_number("PRINT X"), None);
        assert_eq!(parse_line_number("    PRINT X"), None);
    }

    #[test]
    fn it_returns_none_for_hugely_numbered_lines() {
        assert_eq!(
            parse_line_number("99999999999999999999999999 PRINT X"),
            None
        );
    }
}
