#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Identifier(String),
    StringLiteral(String),
    Number(String),
    Symbol(char),
}

pub(crate) struct SourceEvidence {
    tokens: Vec<Token>,
}

impl SourceEvidence {
    pub(crate) fn parse(source: &str) -> Self {
        Self {
            tokens: tokenize(source),
        }
    }

    pub(crate) fn has_identifier(&self, expected: &str) -> bool {
        self.tokens
            .iter()
            .any(|token| is_identifier(token, expected))
    }

    pub(crate) fn has_call(&self, function: &str) -> bool {
        self.tokens
            .windows(2)
            .any(|pair| is_identifier(&pair[0], function) && pair[1] == Token::Symbol('('))
    }

    pub(crate) fn has_section(&self, expected: &str) -> bool {
        self.sections().any(|section| section == expected)
    }

    pub(crate) fn has_program_section(&self) -> bool {
        self.sections()
            .any(|section| section != "license" && !section.starts_with('.'))
    }

    pub(crate) fn returns_identifier(&self, expected: &str) -> bool {
        self.tokens.iter().enumerate().any(|(index, token)| {
            if !is_identifier(token, "return") {
                return false;
            }
            self.tokens[index + 1..]
                .iter()
                .take_while(|candidate| **candidate != Token::Symbol(';'))
                .any(|candidate| is_identifier(candidate, expected))
        })
    }

    pub(crate) fn assigns_call(&self, variable: &str, function: &str) -> bool {
        self.tokens.iter().enumerate().any(|(index, token)| {
            if !is_identifier(token, function)
                || self.tokens.get(index + 1) != Some(&Token::Symbol('('))
            {
                return false;
            }
            let statement_start = self.tokens[..index]
                .iter()
                .rposition(|candidate| matches!(candidate, Token::Symbol(';') | Token::Symbol('{')))
                .map_or(0, |position| position + 1);
            let prefix = &self.tokens[statement_start..index];
            prefix
                .windows(2)
                .any(|pair| is_identifier(&pair[0], variable) && pair[1] == Token::Symbol('='))
        })
    }

    pub(crate) fn has_null_guard(&self, variable: &str) -> bool {
        self.if_conditions()
            .any(|condition| condition_checks_null(condition, variable))
    }

    fn sections(&self) -> impl Iterator<Item = &str> {
        self.tokens.windows(4).filter_map(|tokens| match tokens {
            [Token::Identifier(macro_name), Token::Symbol('('), Token::StringLiteral(section), Token::Symbol(')')]
                if macro_name == "SEC" =>
            {
                Some(section.as_str())
            }
            _ => None,
        })
    }

    fn if_conditions(&self) -> impl Iterator<Item = &[Token]> {
        self.tokens.iter().enumerate().filter_map(|(index, token)| {
            if !is_identifier(token, "if")
                || self.tokens.get(index + 1) != Some(&Token::Symbol('('))
            {
                return None;
            }
            matching_paren(&self.tokens, index + 1).map(|end| &self.tokens[index + 2..end])
        })
    }
}

fn matching_paren(tokens: &[Token], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token {
            Token::Symbol('(') => depth += 1,
            Token::Symbol(')') => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn condition_checks_null(tokens: &[Token], variable: &str) -> bool {
    let flat = tokens
        .iter()
        .filter(|token| !matches!(token, Token::Symbol('(') | Token::Symbol(')')))
        .collect::<Vec<_>>();

    flat.windows(2)
        .any(|pair| *pair[0] == Token::Symbol('!') && is_identifier(pair[1], variable))
        || flat.windows(4).any(|part| {
            (is_identifier(part[0], variable)
                && is_double_equals(part[1], part[2])
                && is_null(part[3]))
                || (is_null(part[0])
                    && is_double_equals(part[1], part[2])
                    && is_identifier(part[3], variable))
        })
}

fn is_identifier(token: &Token, expected: &str) -> bool {
    matches!(token, Token::Identifier(value) if value == expected)
}

fn is_double_equals(first: &Token, second: &Token) -> bool {
    *first == Token::Symbol('=') && *second == Token::Symbol('=')
}

fn is_null(token: &Token) -> bool {
    is_identifier(token, "NULL") || matches!(token, Token::Number(value) if value == "0")
}

fn tokenize(source: &str) -> Vec<Token> {
    let chars = source.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    let mut line_start = true;

    while index < chars.len() {
        let current = chars[index];
        if current == '\n' {
            line_start = true;
            index += 1;
        } else if current.is_whitespace() {
            index += 1;
        } else if line_start && current == '#' {
            index = skip_preprocessor(&chars, index);
            line_start = true;
        } else if current == '/' && chars.get(index + 1) == Some(&'/') {
            index = skip_line(&chars, index + 2);
        } else if current == '/' && chars.get(index + 1) == Some(&'*') {
            let (next, saw_newline) = skip_block_comment(&chars, index + 2);
            index = next;
            line_start = line_start || saw_newline;
        } else {
            line_start = false;
            match current {
                '"' => {
                    let (value, next) = read_quoted(&chars, index + 1, '"');
                    tokens.push(Token::StringLiteral(value));
                    index = next;
                }
                '\'' => {
                    index = read_quoted(&chars, index + 1, '\'').1;
                }
                value if is_identifier_start(value) => {
                    let end = take_while(&chars, index + 1, is_identifier_continue);
                    tokens.push(Token::Identifier(chars[index..end].iter().collect()));
                    index = end;
                }
                value if value.is_ascii_digit() => {
                    let end = take_while(&chars, index + 1, |candidate| {
                        candidate.is_ascii_alphanumeric() || candidate == '_'
                    });
                    tokens.push(Token::Number(chars[index..end].iter().collect()));
                    index = end;
                }
                symbol => {
                    tokens.push(Token::Symbol(symbol));
                    index += 1;
                }
            }
        }
    }
    tokens
}

fn skip_preprocessor(chars: &[char], mut index: usize) -> usize {
    while index < chars.len() {
        if chars[index] == '\n' {
            if index > 0 && chars[index - 1] == '\\' {
                index += 1;
                continue;
            }
            return index + 1;
        }
        index += 1;
    }
    index
}

fn skip_line(chars: &[char], mut index: usize) -> usize {
    while index < chars.len() && chars[index] != '\n' {
        index += 1;
    }
    index
}

fn skip_block_comment(chars: &[char], mut index: usize) -> (usize, bool) {
    let mut saw_newline = false;
    while index < chars.len() {
        saw_newline |= chars[index] == '\n';
        if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
            return (index + 2, saw_newline);
        }
        index += 1;
    }
    (index, saw_newline)
}

fn read_quoted(chars: &[char], mut index: usize, quote: char) -> (String, usize) {
    let mut value = String::new();
    while index < chars.len() {
        match chars[index] {
            '\\' if index + 1 < chars.len() => {
                value.push(chars[index + 1]);
                index += 2;
            }
            current if current == quote => return (value, index + 1),
            current => {
                value.push(current);
                index += 1;
            }
        }
    }
    (value, index)
}

fn take_while(chars: &[char], mut index: usize, predicate: fn(char) -> bool) -> usize {
    while index < chars.len() && predicate(chars[index]) {
        index += 1;
    }
    index
}

fn is_identifier_start(value: char) -> bool {
    value == '_' || value.is_ascii_alphabetic()
}

fn is_identifier_continue(value: char) -> bool {
    is_identifier_start(value) || value.is_ascii_digit()
}
