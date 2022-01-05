/// Token used to keep track of write rights
#[derive(Debug)]
pub struct Token {
    _marker: (),
}

impl Token {
    pub fn mint() -> Self {
        Token { _marker: () }
    }
}

pub struct TokenBuffer<'a> {
    token: Token,
    buffer: &'a mut [u8],
}

impl<'a> TokenBuffer<'a> {
    pub fn new(token: Token, buffer: &'a mut [u8]) -> Self {
        TokenBuffer { token, buffer }
    }

    pub fn consume(self) -> Token {
        self.token
    }
}

impl<'a> AsMut<[u8]> for TokenBuffer<'a> {
    fn as_mut(&mut self) -> &mut [u8] {
        self.buffer
    }
}
