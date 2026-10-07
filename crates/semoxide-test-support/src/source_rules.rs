//! Source rules checked on Rust tokens (`proc-macro2`), so comments and string contents never
//! count: `#![forbid(unsafe_code)]` in crate roots (CLAUDE.md Rust rule 6), no inline test
//! modules (CODE-ARCHITECTURE §8), private tuple fields (Rust rule 4).

use std::io;
use std::path::{Path, PathBuf};

use proc_macro2::{Delimiter, TokenStream, TokenTree};

#[cfg(test)]
mod tests;

/// Whether the file has a `#![forbid(…)]` inner attribute that lists `unsafe_code`.
#[must_use]
pub fn forbids_unsafe_code(text: &str) -> bool {
    tokens(text).windows(3).any(|window| {
        matches!(window, [TokenTree::Punct(hash), TokenTree::Punct(bang), TokenTree::Group(attribute)]
            if hash.as_char() == '#'
                && bang.as_char() == '!'
                && attribute.delimiter() == Delimiter::Bracket
                && lists_lint(attribute.stream(), "forbid", "unsafe_code"))
    })
}

/// Names of modules declared `#[cfg(test)] mod name { … }` with a body instead of `mod name;`.
#[must_use]
pub fn inline_test_modules(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    collect_inline_test_modules(&tokens(text), &mut names);
    names
}

/// Names of tuple structs with a `pub` field, e.g. `struct Tag(pub String)`.
#[must_use]
pub fn pub_tuple_fields(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    collect_pub_tuple_fields(&tokens(text), &mut names);
    names
}

/// The top-level tokens; text that isn't valid Rust tokens has none.
fn tokens(text: &str) -> Vec<TokenTree> {
    text.parse::<TokenStream>()
        .map(|stream| stream.into_iter().collect())
        .unwrap_or_default()
}

/// `level(…, lint, …)`, e.g. `forbid(missing_docs, unsafe_code)`.
fn lists_lint(attribute: TokenStream, level: &str, lint: &str) -> bool {
    let attribute: Vec<TokenTree> = attribute.into_iter().collect();
    matches!(attribute.as_slice(), [TokenTree::Ident(name), TokenTree::Group(lints)]
        if name == level
            && lints.delimiter() == Delimiter::Parenthesis
            && lints.stream().into_iter().any(|token| matches!(token, TokenTree::Ident(ident) if ident == lint)))
}

/// `#[…]` starting at `index`: the attribute's tokens.
fn outer_attribute(tokens: &[TokenTree], index: usize) -> Option<TokenStream> {
    match tokens.get(index..index + 2)? {
        [TokenTree::Punct(hash), TokenTree::Group(attribute)]
            if hash.as_char() == '#' && attribute.delimiter() == Delimiter::Bracket =>
        {
            Some(attribute.stream())
        }
        _ => None,
    }
}

fn is_cfg_test(attribute: TokenStream) -> bool {
    let attribute: Vec<TokenTree> = attribute.into_iter().collect();
    matches!(attribute.as_slice(), [TokenTree::Ident(cfg), TokenTree::Group(condition)]
        if cfg == "cfg"
            && condition.delimiter() == Delimiter::Parenthesis
            && condition.stream().to_string() == "test")
}

fn is_ident(token: Option<&TokenTree>, name: &str) -> bool {
    matches!(token, Some(TokenTree::Ident(ident)) if ident == name)
}

fn collect_inline_test_modules(tokens: &[TokenTree], names: &mut Vec<String>) {
    for (index, token) in tokens.iter().enumerate() {
        if let TokenTree::Group(group) = token {
            let inner: Vec<TokenTree> = group.stream().into_iter().collect();
            collect_inline_test_modules(&inner, names);
        }
        if !outer_attribute(tokens, index).is_some_and(is_cfg_test) {
            continue;
        }
        // Skip further attributes and a visibility, then expect `mod name { … }`.
        let mut next = index + 2;
        while outer_attribute(tokens, next).is_some() {
            next += 2;
        }
        if is_ident(tokens.get(next), "pub") {
            next += 1;
            if matches!(tokens.get(next), Some(TokenTree::Group(scope)) if scope.delimiter() == Delimiter::Parenthesis)
            {
                next += 1;
            }
        }
        if let (true, Some(TokenTree::Ident(name)), Some(TokenTree::Group(body))) = (
            is_ident(tokens.get(next), "mod"),
            tokens.get(next + 1),
            tokens.get(next + 2),
        ) && body.delimiter() == Delimiter::Brace
        {
            names.push(name.to_string());
        }
    }
}

fn collect_pub_tuple_fields(tokens: &[TokenTree], names: &mut Vec<String>) {
    for (index, token) in tokens.iter().enumerate() {
        if let TokenTree::Group(group) = token {
            let inner: Vec<TokenTree> = group.stream().into_iter().collect();
            collect_pub_tuple_fields(&inner, names);
        }
        let (true, Some(TokenTree::Ident(name))) =
            (is_ident(Some(token), "struct"), tokens.get(index + 1))
        else {
            continue;
        };
        // The first group after the name (past any generics) holds the fields; a unit struct
        // ends at `;` before any group.
        let fields = tokens
            .iter()
            .skip(index + 2)
            .take_while(
                |token| !matches!(token, TokenTree::Punct(semicolon) if semicolon.as_char() == ';'),
            )
            .find_map(|token| match token {
                TokenTree::Group(group) => Some(group),
                _ => None,
            });
        if let Some(fields) = fields
            && fields.delimiter() == Delimiter::Parenthesis
            && has_pub_field(fields.stream())
        {
            names.push(name.to_string());
        }
    }
}

/// Whether any comma-separated field, after its attributes, starts with `pub`.
fn has_pub_field(fields: TokenStream) -> bool {
    let tokens: Vec<TokenTree> = fields.into_iter().collect();
    tokens
        .split(|token| matches!(token, TokenTree::Punct(comma) if comma.as_char() == ','))
        .any(|field| {
            let mut start = 0;
            while outer_attribute(field, start).is_some() {
                start += 2;
            }
            is_ident(field.get(start), "pub")
        })
}

/// Every `.rs` file under `crates/*/src/`, with its content.
///
/// # Errors
///
/// Returns an I/O error if a directory or file can't be read.
pub fn crate_sources(crates_dir: &Path) -> io::Result<Vec<(PathBuf, String)>> {
    let mut sources = Vec::new();
    for entry in std::fs::read_dir(crates_dir)? {
        let src = entry?.path().join("src");
        if src.is_dir() {
            collect_rust_files(&src, &mut sources)?;
        }
    }
    Ok(sources)
}

fn collect_rust_files(dir: &Path, sources: &mut Vec<(PathBuf, String)>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_rust_files(&path, sources)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let text = std::fs::read_to_string(&path)?;
            sources.push((path, text));
        }
    }
    Ok(())
}
