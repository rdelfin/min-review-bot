use logos::{Logos, Span};
/// This file is responsible for parsing the codeowners file and returning a logical struct from
/// it which we can use to later evaluate ownership rules.
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct CodeOwners {
    rules: Vec<OwnerRule>,
}

#[derive(Debug, Clone)]
pub struct OwnerRule {
    pattern: String,
    owners: BTreeSet<String>,
}

impl CodeOwners {
    pub fn parse(data: String) -> Result<CodeOwners> {
        let mut rules = vec![];
        let lexed_output = lex_codeowners(&data);
        let filtered_lex = lexed_output
            .into_iter()
            .filter(|(token, _, _)| *token != CodeOwnersToken::Comment);

        let mut state = ParseState::NoRule;
        for (token, text, span) in filtered_lex {
            state = match state {
                ParseState::NoRule => match token {
                    CodeOwnersToken::Owner => {
                        return Err(ParseError::OwnerStartLine(span.start, text.into()));
                    }
                    CodeOwnersToken::Path => ParseState::HasPath { path: text.into() },
                    CodeOwnersToken::NewLine => ParseState::NoRule,
                    CodeOwnersToken::Error => {
                        return Err(ParseError::LexFailure(span.start, text.into()));
                    }
                    // Should be filtered out
                    CodeOwnersToken::Comment => {
                        unreachable!();
                    }
                },
                ParseState::HasPath { path } => match token {
                    CodeOwnersToken::Owner => ParseState::PathWithOwners {
                        path,
                        owners: BTreeSet::from([text.into()]),
                    },
                    CodeOwnersToken::Path => {
                        return Err(ParseError::DoublePath(span.start, text.into()));
                    }
                    // Believe it or not, explicitly unowned files are a thing allowed by codeowners
                    CodeOwnersToken::NewLine => ParseState::NoRule,
                    CodeOwnersToken::Error => {
                        return Err(ParseError::LexFailure(span.start, text.into()));
                    }
                    // Should be filtered out
                    CodeOwnersToken::Comment => {
                        unreachable!();
                    }
                },
                ParseState::PathWithOwners { path, mut owners } => match token {
                    CodeOwnersToken::Owner => {
                        owners.insert(text.into());
                        ParseState::PathWithOwners { path, owners }
                    }
                    CodeOwnersToken::Path => {
                        return Err(ParseError::PathAfterOwner(span.start, path));
                    }
                    CodeOwnersToken::NewLine => {
                        rules.push(OwnerRule {
                            pattern: path,
                            owners,
                        });
                        ParseState::NoRule
                    }
                    CodeOwnersToken::Error => {
                        return Err(ParseError::LexFailure(span.start, text.into()));
                    }
                    // Should be filtered out
                    CodeOwnersToken::Comment => {
                        unreachable!();
                    }
                },
            };
        }

        Ok(CodeOwners { rules })
    }

    pub fn owners<'a>(&'a self, file_path: &str) -> Option<Vec<&'a str>> {
        let mut rule_idx = None;
        for (i, rule) in self.rules.iter().enumerate() {
            if matches_pattern(file_path, &rule.pattern) {
                rule_idx = Some(i);
            }
        }

        Some(
            self.rules[rule_idx?]
                .owners
                .iter()
                .map(|s| s.as_ref())
                .collect(),
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("at character {0} expected path, got owner {1}")]
    OwnerStartLine(usize, String),
    #[error("at character {0} expected owners, got another path {1}")]
    DoublePath(usize, String),
    #[error("at character {0}, for path {1}, expected more owners, but got a path instead")]
    PathAfterOwner(usize, String),
    #[error("at character {0} got lex error for string {1}")]
    LexFailure(usize, String),
}

pub type Result<T = (), E = ParseError> = std::result::Result<T, E>;

#[derive(Debug, PartialEq, Clone)]
enum ParseState {
    NoRule,
    HasPath {
        path: String,
    },
    PathWithOwners {
        path: String,
        owners: BTreeSet<String>,
    },
}

fn lex_codeowners<'a>(data: &'a str) -> Vec<(CodeOwnersToken, &'a str, Span)> {
    let mut lexed_output = vec![];
    let mut lex = CodeOwnersToken::lexer(&data);
    loop {
        let next = match lex.next() {
            None => {
                break;
            }
            Some(next) => next,
        };
        lexed_output.push((next, lex.slice(), lex.span()));
    }

    lexed_output
}

#[derive(Logos, Debug, PartialEq, Clone, Copy)]
enum CodeOwnersToken {
    #[regex(r"#[^\n]*")]
    Comment,
    #[regex(r"\n")]
    NewLine,
    #[regex(r"@[A-Za-z0-9/\-_]+")]
    Owner,
    #[regex(r"[A-Za-z0-9.*!@$%^&*(){}\[\]/_\-]+")]
    Path,
    #[error]
    // We can also use this variant to define whitespace,
    // or any other matches we wish to skip.
    #[regex(r"[ \t\f]+", logos::skip)]
    Error,
}

fn matches_pattern(file_path: &str, pattern: &str) -> bool {
    true
}
