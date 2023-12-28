/// First-generation BASIC dialects completely ignored spaces
/// and tabs. This is part of what made it possible to write
/// either `GO TO` or `GOTO`, for instance.
///
/// This struct allows clients to iterate through the bytes
/// of an array, skipping all such whitespace.
pub struct LineCruncher<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl<'a> LineCruncher<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        LineCruncher { bytes, index: 0 }
    }

    pub fn is_basic_whitespace(byte: u8) -> bool {
        byte.is_ascii_whitespace() && byte != b'\n'
    }

    /// Returns the total number of bytes that have been consumed
    /// so far, including whitespace.
    pub fn pos(&self) -> usize {
        self.index
    }
}

impl<'a> Iterator for LineCruncher<'a> {
    /// A tuple of the byte and the total number of bytes consumed
    /// so far, including the given byte and any prior whitespace.
    type Item = (u8, usize);

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.bytes.len() {
            let byte = self.bytes[self.index];
            self.index += 1;
            if !LineCruncher::is_basic_whitespace(byte) {
                return Some((byte, self.index));
            }
        }

        None
    }
}
