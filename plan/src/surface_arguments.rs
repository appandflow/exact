//! The named arguments a GPU product's surfaces accept, as the product itself
//! carries them, so a build can compare them with the plan it packages beside
//! the product (expo/exact#176).
//!
//! A surface's arguments are bound by name at run time (`exact_gpu`'s
//! `bind_json`), so a canvas passing a name the product lacks is otherwise
//! found only when the canvas binds. The product is compiled before the
//! Contract bake reads its sources, and an edit between the two leaves a plan
//! and a product that disagree. A product whose surfaces' arguments are known
//! at compile time (a game's `Args::FIELDS`) carries one [`record`] per
//! surface in its data; the bake finds them in the product's bytes
//! ([`declared`]) and refuses a plan that passes an undeclared name
//! ([`undeclared`]). A surface without a record is not checked.
//!
//! A record is the marker, then the surface's name and each argument's name,
//! each followed by NUL, then one more NUL. No name is empty or contains NUL,
//! so a record reads back one way.

use crate::{Plan, SurfaceArgsMode};
use std::collections::BTreeMap;

/// What begins every record.
pub const MARKER: &[u8] = b"\0exact-gpu-surface-arguments-v1\0";

/// The length of `surface`'s record over `fields` (each an argument's name
/// and whatever the declaring type pairs with it), for the array [`record`]
/// fills.
pub const fn record_len<K>(surface: &str, fields: &[(&str, K)]) -> usize {
    let mut len = MARKER.len() + surface.len() + 2;
    let mut i = 0;
    while i < fields.len() {
        len += fields[i].0.len() + 1;
        i += 1;
    }
    len
}

/// `surface`'s record over `fields`; `N` is [`record_len`] of the same.
pub const fn record<K, const N: usize>(surface: &str, fields: &[(&str, K)]) -> [u8; N] {
    let mut out = [0u8; N];
    let mut at = copy(&mut out, 0, MARKER);
    at = copy(&mut out, at, surface.as_bytes()) + 1;
    let mut i = 0;
    while i < fields.len() {
        at = copy(&mut out, at, fields[i].0.as_bytes()) + 1;
        i += 1;
    }
    assert!(at + 1 == N, "record length differs from record_len");
    out
}

const fn copy<const N: usize>(out: &mut [u8; N], mut at: usize, bytes: &[u8]) -> usize {
    let mut i = 0;
    while i < bytes.len() {
        out[at] = bytes[i];
        at += 1;
        i += 1;
    }
    at
}

/// Every record in `bytes` (a product file), as surface name to argument
/// names in bind order. A record that does not parse, or two records naming
/// one surface differently, is refused.
pub fn declared(bytes: &[u8]) -> Result<BTreeMap<String, Vec<String>>, String> {
    // The marker is assembled here rather than written as one literal, so
    // this function's own bytes, linked into a product, are never a record.
    let marker = [&MARKER[..1], &MARKER[1..]].concat();
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut from = 0;
    while let Some(at) = find(&bytes[from..], &marker) {
        let mut rest = &bytes[from + at + marker.len()..];
        let mut field = || -> Result<&[u8], String> {
            let end = rest
                .iter()
                .position(|b| *b == 0)
                .ok_or("a surface argument record has no end")?;
            let (name, tail) = rest.split_at(end);
            rest = &tail[1..];
            Ok(name)
        };
        let surface = text(field()?, false)?;
        let mut names = Vec::new();
        loop {
            let name = field()?;
            if name.is_empty() {
                break;
            }
            names.push(text(name, true)?);
        }
        if found.get(&surface).is_some_and(|known| *known != names) {
            return Err(format!(
                "the GPU product declares surface `{surface}`'s arguments twice, differently"
            ));
        }
        found.insert(surface, names);
        from = bytes.len() - rest.len();
    }
    Ok(found)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// A record's name: printable UTF-8, and for an argument an identifier.
fn text(bytes: &[u8], identifier: bool) -> Result<String, String> {
    let name = std::str::from_utf8(bytes)
        .ok()
        .filter(|s| !s.is_empty() && !s.chars().any(char::is_control))
        .filter(|s| {
            !identifier
                || (s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                    && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        });
    name.map(str::to_owned)
        .ok_or_else(|| format!("a surface argument record names {bytes:?}"))
}

/// Each `(surface, argument)` a canvas of `plan` passes by name that
/// `declared` gives its surface no argument of, in plan order, once each.
pub fn undeclared(plan: &Plan, declared: &BTreeMap<String, Vec<String>>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for surface in plan.surfaces.iter() {
        let name = plan.str(surface.name);
        let Some(known) = declared.get(name) else {
            continue;
        };
        if surface.mode != SurfaceArgsMode::Named {
            continue;
        }
        for id in surface.args.iter() {
            let arg = plan.str(plan.surface_arg(id).name);
            let pair = (name.to_owned(), arg.to_owned());
            if !known.iter().any(|k| k == arg) && !out.contains(&pair) {
                out.push(pair);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIELDS: &[(&str, u8)] = &[("count", 0), ("session_carry", 1)];
    const LEN: usize = record_len("dice", FIELDS);
    static RECORD: [u8; LEN] = record("dice", FIELDS);

    #[test]
    fn a_record_is_found_among_other_bytes() {
        let product = [b"\x7fELF padding".as_slice(), &RECORD, b"tail\0\0"].concat();
        let found = declared(&product).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found["dice"], ["count", "session_carry"]);
        const NONE: &[(&str, u8)] = &[];
        let empty: [u8; record_len("still", NONE)] = record("still", NONE);
        let both = [product.as_slice(), &empty, &RECORD].concat();
        let found = declared(&both).unwrap();
        assert_eq!(found["still"], Vec::<String>::new());
        assert_eq!(found["dice"], ["count", "session_carry"]);
        assert!(declared(b"no records here").unwrap().is_empty());
    }

    #[test]
    fn a_malformed_or_contradicting_record_is_refused() {
        let other: [u8; record_len("dice", &[("count", ())])] = record("dice", &[("count", ())]);
        let twice = [RECORD.as_slice(), &other].concat();
        assert!(declared(&twice).unwrap_err().contains("twice"));
        let cut = &RECORD[..RECORD.len() - 3];
        assert!(declared(cut).unwrap_err().contains("no end"));
        let bad = [MARKER, b"dice\0not an identifier\0\0"].concat();
        assert!(declared(&bad).unwrap_err().contains("names"));
    }

    #[test]
    fn a_name_the_record_lacks_is_undeclared() {
        let mut b = crate::builder::PlanBuilder::new(0, 0);
        let one = b.constant(&crate::Value::Number(1.));
        b.surface("dice", &[("count", one), ("extra", one)]);
        b.surface("dice", &[("extra", one)]);
        b.surface("chart", &[("series", one)]);
        let plan = b.plan();
        let found = declared(&RECORD).unwrap();
        assert_eq!(
            undeclared(plan, &found),
            [("dice".to_owned(), "extra".to_owned())]
        );
        let all: [u8; record_len("dice", &[("count", ()), ("extra", ())])] =
            record("dice", &[("count", ()), ("extra", ())]);
        assert!(undeclared(plan, &declared(&all).unwrap()).is_empty());
    }
}
