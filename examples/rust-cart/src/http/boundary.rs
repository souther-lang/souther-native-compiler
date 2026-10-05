//! How the boundary's decoders and the model's are put together over one value.
//!
//! A raoh decoder writes a value in the form its reader wants and refuses what cannot be written
//! so: it canonicalises and validates as one step, and what it refuses has no value. The boundary
//! decodes what it owns of a request that way (an id in lower case and shaped like a UUID, an email
//! trimmed, lowercased and shaped like one, a name trimmed), and the model's decoder reads what
//! those answered.
//!
//! Where the model reads a value whole and the boundary owns some of its members, each member is
//! decoded on its own, and the model reads the value whichever of them was refused, so the issues
//! of both come back together. What the model is handed of a member is what the boundary's decoder
//! answered for it, and of a refused member nothing: the model never reads text the boundary
//! refused, so what it sees does not depend on whether the rest of the request was valid. It then
//! reports the refused member as missing, which is only that it was taken out, and that one issue
//! is dropped: the one at the member's own path. Nothing inside the member is the model's to
//! report, since the model was not handed it; what is wrong inside it is its decoder's to say.

use raoh::json::prelude::*;
use raoh::json::{JsonObject, Lexeme, View};
use raoh::{Issues, Path, Pointer, decoder_fn};

/// A member of a value the model reads whole, and the boundary's decoder of it.
pub type Member = (&'static str, BoxDecoder<Json, Node>);

/// A value read whole by `model` once each of `members` it has is decoded by the boundary.
pub fn members<'a, T>(
    members: Vec<Member>,
    model: impl Decoder<Json, Output = T> + 'a,
) -> impl Decoder<Json, Output = T> + 'a {
    decoder_fn(move |given: &Json, path: &Path<'_>| {
        let mut decoded = owned(given);
        let mut found = Issues::new();
        let mut refused: Vec<Pointer> = Vec::new();
        if let Node::Object(object) = &decoded {
            let mut kept = Vec::with_capacity(object.len());
            for (name, member) in object.iter() {
                let Some((_, decoder)) = members.iter().find(|(it, _)| *it == name) else {
                    kept.push((name.to_owned(), member.clone()));
                    continue;
                };
                let at = path.key(name);
                match decoder.decode_at(member, &at) {
                    Ok(written) => kept.push((name.to_owned(), written)),
                    Err(issues) => {
                        refused.push(at.to_pointer());
                        found.merge(issues);
                    }
                }
            }
            decoded = Node::Object(JsonObject::new(kept).expect("the names of one object"));
        }
        match model.decode_at(&decoded, path) {
            Ok(read) if found.is_empty() => Ok(read),
            Ok(_) => Err(found),
            Err(read) => {
                for issue in read.iter() {
                    if !refused.contains(issue.path()) {
                        found.push(issue.clone());
                    }
                }
                Err(found)
            }
        }
    })
}

/// What a decoder was handed, as a value of its own the boundary can take members out of and put
/// members into: each number as the text it was written as.
fn owned(given: &Json) -> Node {
    match given.view() {
        View::Missing | View::Null => Node::Null,
        View::Bool(truth) => Node::Bool(truth),
        View::Number(number) => {
            Node::Number(Lexeme::new(&number.lexeme()).expect("a number of the input model"))
        }
        View::String(text) => Node::String(text.to_owned()),
        View::Array(elements) => Node::Array(elements.iter().map(owned).collect()),
        View::Object(object) => {
            let mut kept = Vec::with_capacity(object.len());
            object.each(&mut |name, value| kept.push((name.to_owned(), owned(value))));
            Node::Object(JsonObject::new(kept).expect("the names of one object"))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use raoh::Issue;

    use super::*;

    /// A value written as JSON, to say what the model was handed.
    fn written(node: &Node) -> String {
        match node {
            Node::Null => "null".to_owned(),
            Node::Bool(truth) => truth.to_string(),
            Node::Number(lexeme) => lexeme.to_string(),
            Node::String(text) => serde_json::to_string(text).unwrap(),
            Node::Array(items) => {
                format!("[{}]", items.iter().map(written).collect::<Vec<_>>().join(","))
            }
            Node::Object(object) => format!(
                "{{{}}}",
                object
                    .iter()
                    .map(|(name, value)| format!(
                        "{}:{}",
                        serde_json::to_string(name).unwrap(),
                        written(value)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }

    /// A model that records what it was handed, and reports each of `required` it is missing.
    fn recording<'r>(
        read: &'r RefCell<Vec<String>>,
        required: &'r [&'r str],
    ) -> impl Decoder<Json, Output = ()> + 'r {
        decoder_fn(move |given: &Json, path: &Path<'_>| {
            read.borrow_mut().push(written(&owned(given)));
            let View::Object(object) = given.view() else {
                return Err(Issue::new("type_mismatch").at(path.to_pointer()).into());
            };
            let mut missing = Issues::new();
            for name in required {
                if object.get(name).is_none() {
                    missing.push(Issue::new("missing_field").at(path.key(name).to_pointer()));
                }
            }
            if missing.is_empty() {
                Ok(())
            } else {
                Err(missing)
            }
        })
    }

    fn email() -> Member {
        (
            "email",
            string()
                .trim()
                .lowercase()
                .email()
                .map(Node::String)
                .boxed(),
        )
    }

    fn paths(issues: &Issues) -> Vec<String> {
        issues
            .iter()
            .map(|issue| issue.path().to_string())
            .collect()
    }

    #[test]
    fn the_model_reads_what_the_boundary_wrote() {
        let read = RefCell::new(Vec::new());

        members(vec![email()], recording(&read, &["email"]))
            .decode(&json!({ "email": " A@Example.COM " }))
            .unwrap();

        assert_eq!(read.into_inner(), [r#"{"email":"a@example.com"}"#]);
    }

    #[test]
    fn the_model_is_never_handed_what_the_boundary_refused() {
        let read = RefCell::new(Vec::new());

        let issues = members(vec![email()], recording(&read, &["email", "city"]))
            .decode(&json!({ "email": " X ", "city": "Tokyo" }))
            .unwrap_err();

        assert_eq!(read.into_inner(), [r#"{"city":"Tokyo"}"#]);
        assert_eq!(paths(&issues), ["/email"]);
    }

    #[test]
    fn the_model_reads_the_rest_whichever_member_was_refused() {
        let read = RefCell::new(Vec::new());

        let issues = members(vec![email()], recording(&read, &["email", "city"]))
            .decode(&json!({ "email": "not-an-email" }))
            .unwrap_err();

        assert_eq!(paths(&issues), ["/email", "/city"]);
    }

    #[test]
    fn a_number_reaches_the_model_as_it_was_written() {
        let read = RefCell::new(Vec::new());

        from_str(&members(vec![email()], recording(&read, &[])), r#"{"price":1.50}"#).unwrap();

        assert_eq!(read.into_inner(), [r#"{"price":1.50}"#]);
    }

    #[test]
    fn only_the_refused_members_own_path_is_taken_for_the_boundarys() {
        // The boundary's decoder of an address refuses its postcode, and the address is taken out.
        // The model reports it missing, which is dropped, and the model's own issue beside it is
        // kept: nothing at a path merely under or beside the refused member is dropped.
        let address = decoder_fn(|_: &Json, path: &Path<'_>| -> Result<Node, Issues> {
            Err(Issue::new("invalid_format")
                .at(path.key("postcode").to_pointer())
                .into())
        });
        let read = RefCell::new(Vec::new());

        let issues = members(
            vec![("address", address.boxed())],
            recording(&read, &["address", "addressee"]),
        )
        .decode(&json!({ "address": { "postcode": 1 } }))
        .unwrap_err();

        assert_eq!(paths(&issues), ["/address/postcode", "/addressee"]);
    }
}
