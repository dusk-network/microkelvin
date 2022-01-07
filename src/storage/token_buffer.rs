use rkyv::{ser::Serializer, Fallible};

#[derive(Debug)]
pub enum Token {
    Active,
    Vacant,
}

impl Token {
    pub fn new() -> Self {
        Token::Active
    }

    pub fn vacant(&self) -> bool {
        match self {
            Token::Active => false,
            Token::Vacant => true,
        }
    }

    pub fn take(&mut self) -> Option<Token> {
        match self {
            Token::Active => {
                *self = Token::Vacant;
                Some(Token::Active)
            }
            Token::Vacant => None,
        }
    }

    pub fn return_token(&mut self, token: Token) {
        debug_assert!(self.vacant());
        debug_assert!(!token.vacant());
        *self = token;
    }
}

pub struct TokenBuffer {
    token: Token,
    buffer: *mut [u8],
    written: usize,
}

impl TokenBuffer {
    pub fn new(token: Token, buffer: &mut [u8]) -> Self {
        TokenBuffer {
            token,
            buffer,
            written: 0,
        }
    }

    pub(crate) fn placeholder() -> Self {
        TokenBuffer {
            token: Token::new(),
            buffer: &mut [],
            written: 0,
        }
    }

    pub fn consume(self) -> Token {
        self.token
    }

    pub fn written_bytes(&self) -> &[u8] {
        let slice = unsafe { &*self.buffer };
        &slice[..self.written]
    }

    pub fn unwritten_bytes(&mut self) -> &mut [u8] {
        let slice = unsafe { &mut *self.buffer };
        &mut slice[self.written..]
    }

    pub fn advance(&mut self) -> usize {
        let written = self.written;
        self.buffer = &mut unsafe { &mut *self.buffer }[written..];
        self.written = 0;
        written
    }

    pub fn remap(&mut self, buffer: &mut [u8]) {
        self.buffer = buffer;
        self.written = 0;
    }
}

pub struct BufferOverflow;

impl Serializer for TokenBuffer {
    fn pos(&self) -> usize {
        self.written
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        let remaining_buffer = self.unwritten_bytes();
        let bytes_length = bytes.len();
        if remaining_buffer.len() >= bytes_length {
            remaining_buffer[..bytes_length].copy_from_slice(bytes);
            self.written += bytes_length;
            Ok(())
        } else {
            Err(BufferOverflow)
        }
    }
}

impl Fallible for TokenBuffer {
    type Error = BufferOverflow;
}

impl AsMut<[u8]> for TokenBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        unsafe { &mut *self.buffer }
    }
}
