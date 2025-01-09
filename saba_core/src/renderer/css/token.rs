use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, PartialEq)]
pub enum CssToken {
    /// https://www.w3.org/TR/css-syntax-3/#typedef-hash-token
    HashToken(String),
    /// https://www.w3.org/TR/css-syntax-3/#typedef-delim-token
    Delim(char),
    /// https://www.w3.org/TR/css-syntax-3/#typedef-number-token
    Number(f64),
    /// https://www.w3.org/TR/css-syntax-3/#typedef-colon-token
    Colon,
    /// https://www.w3.org/TR/css-syntax-3/#typedef-semicolon-token
    SemiColon,
    /// https://www.w3.org/TR/css-syntax-3/#typedef-open-token
    OpenParenthesis,
    /// https://www.w3.org/TR/css-syntax-3/#typedef-close-token
    ColseParenthesis,
    /// https://www.w3.org/TR/css-syntax-3/#typedef-open-curly
    OpenCurly,
    /// https://www.w3.org/TR/css-syntax-3/#typedef-close-curly
    CloseCurly,
    /// https://www.w3.org/TR/css-syntax-3/#typedef-ident-curly
    Ident(String),
    /// https://www.w3.org/TR/css-syntax-3/#typedef-string-curly
    StringToken(String),
    /// https://www.w3.org/TR/css-syntax-3/#typedef-at-keyword-token
    AtKeyword(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CssTokenizer {
    pos: usize,
    input: Vec<char>,
}

impl CssTokenizer {
    pub fn new(css: String) -> Self {
        Self {
            pos: 0,
            input: css.chars().collect(),
        }
    }

    /// https://www.w3.org/TR/css-syntax-3/#consume-a-string-token
    fn consume_string_token(&mut self) -> String {
        let mut s = String::new();

        loop {
            if self.pos >= self.input.len() {
                return s;
            }

            self.pos += 1;
            let c = self.input[self.pos];
            match c {
                '"' | '\'' => break,
                _ => s.push(c),
            }
        }

        s
    }

    /// https://www.w3.org/TR/css-syntax-3/#consume-number
    fn consure_numeric_token(&mut self) -> f64 {
        let mut num = 0f64;
        let mut floating = false;
        let mut floating_digit = 1f64;

        loop {
            if self.pos >= self.input.len() {
                return num;
            }

            let c = self.input[self.pos];

            match c {
                '0'..='9' => {
                    if floating {
                        floating_digit *= 1f64 / 10f64;
                        num += (c.to_digit(10).unwrap() as f64) * floating_digit;
                    } else {
                        num = num * 10.0 + (c.to_digit(10).unwrap() as f64);
                    }
                    self.pos += 1;
                }
                '.' => {
                    floating = true;
                    self.pos += 1;
                }
                _ => break,
            }
        }

        num
    }

    /// https://www.w3.org/TR/css-syntax-3/#consume-ident-like-token
    /// https://www.w3.org/TR/css-syntax-3/#consume-name
    fn consume_ident_token(&mut self) -> String {
        let mut s = String::new();
        s.push(self.input[self.pos]);

        loop {
            self.pos += 1;
            let c = self.input[self.pos];
            match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => {
                    s.push(c);
                }
                _ => break,
            }
        }

        s
    }
}

impl Iterator for CssTokenizer {
    type Item = CssToken;

    /// https://www.w3.org/TR/css-syntax-3/#consume-token
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.pos >= self.input.len() {
                return None;
            }

            let c = self.input[self.pos];

            let token = match c {
                '(' => CssToken::OpenParenthesis,
                ')' => CssToken::ColseParenthesis,
                ',' => CssToken::Delim(','),
                '.' => CssToken::Delim('.'),
                ':' => CssToken::Colon,
                ';' => CssToken::SemiColon,
                '{' => CssToken::OpenCurly,
                '}' => CssToken::CloseCurly,
                ' ' | '\n' => {
                    self.pos += 1;
                    continue;
                }
                '"' | '\'' => {
                    let value = self.consume_string_token();
                    CssToken::StringToken(value)
                }
                '#' => {
                    // この実装では、常に #IDの形式のIDセレクタとして扱う。
                    let value = self.consume_ident_token();
                    self.pos -= 1;
                    CssToken::HashToken(value)
                }
                '-' => {
                    // この実装では、負の数を取り扱わないため、ハイフンは識別子の一つとして扱う。
                    let t = CssToken::Ident(self.consume_ident_token());
                    self.pos -= 1;
                    t
                }
                '@' => {
                    // 次の3文字が識別子として有効な文字の場合、<at-keyword-token>を作成して返す。
                    // それ以外の場合、<delim-token>を返す。
                    if self.input[self.pos + 1].is_ascii_alphabetic()
                        && self.input[self.pos + 2].is_ascii_alphabetic()
                        && self.input[self.pos + 3].is_ascii_alphabetic()
                    {
                        // skip @
                        self.pos += 1;
                        let t = CssToken::AtKeyword(self.consume_ident_token());
                        self.pos -= 1;
                        t
                    } else {
                        CssToken::Delim('@')
                    }
                }
                'a'..='z' | 'A'..='Z' | '_' => {
                    let t = CssToken::Ident(self.consume_ident_token());
                    self.pos -= 1;
                    t
                }
                '0'..='9' => {
                    let t = CssToken::Number(self.consure_numeric_token());
                    self.pos -= 1;
                    t
                }
                _ => {
                    unimplemented!("char {} is not supported yet", c);
                }
            };

            self.pos += 1;
            return Some(token);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn test_empty() {
        let style = "".to_string();
        let mut t = CssTokenizer::new(style);
        assert!(t.next().is_none());
    }

    #[test]
    fn test_one_rule() {
        let style = "p { color: red; }".to_string();
        let mut t = CssTokenizer::new(style);
        let expected = [
            CssToken::Ident("p".to_string()),
            CssToken::OpenCurly,
            CssToken::Ident("color".to_string()),
            CssToken::Colon,
            CssToken::Ident("red".to_string()),
            CssToken::SemiColon,
            CssToken::CloseCurly,
        ];
        for e in expected {
            assert_eq!(Some(e.clone()), t.next());
        }
        assert!(t.next().is_none());
    }

    #[test]
    fn test_id_selector() {
        let style = "#id { color: red; }".to_string();
        let mut t = CssTokenizer::new(style);
        let expected = [
            CssToken::HashToken("#id".to_string()),
            CssToken::OpenCurly,
            CssToken::Ident("color".to_string()),
            CssToken::Colon,
            CssToken::Ident("red".to_string()),
            CssToken::SemiColon,
            CssToken::CloseCurly,
        ];
        for e in expected {
            assert_eq!(Some(e.clone()), t.next());
        }
        assert!(t.next().is_none());
    }

    #[test]
    fn test_class_selector() {
        let style = ".class { color: red; }".to_string();
        let mut t = CssTokenizer::new(style);
        let expected = [
            CssToken::Delim('.'),
            CssToken::Ident("class".to_string()),
            CssToken::OpenCurly,
            CssToken::Ident("color".to_string()),
            CssToken::Colon,
            CssToken::Ident("red".to_string()),
            CssToken::SemiColon,
            CssToken::CloseCurly,
        ];
        for e in expected {
            assert_eq!(Some(e.clone()), t.next());
        }
        assert!(t.next().is_none());
    }

    #[test]
    fn test_multiple_rules() {
        let style = "p { content: \"Hey\"; } h1 { font-size: 40; color: blue; }".to_string();
        let mut t = CssTokenizer::new(style);
        let expected = [
            CssToken::Ident("p".to_string()),
            CssToken::OpenCurly,
            CssToken::Ident("content".to_string()),
            CssToken::Colon,
            CssToken::StringToken("Hey".to_string()),
            CssToken::SemiColon,
            CssToken::CloseCurly,
            CssToken::Ident("h1".to_string()),
            CssToken::OpenCurly,
            CssToken::Ident("font-size".to_string()),
            CssToken::Colon,
            CssToken::Number(40.0),
            CssToken::SemiColon,
            CssToken::Ident("color".to_string()),
            CssToken::Colon,
            CssToken::Ident("blue".to_string()),
            CssToken::SemiColon,
            CssToken::CloseCurly,
        ];
        for e in expected {
            assert_eq!(Some(e.clone()), t.next());
        }
        assert!(t.next().is_none());
    }
}
