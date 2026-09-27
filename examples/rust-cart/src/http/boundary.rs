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
use raoh::{Issues, Path, Pointer, decoder_fn};

/// A member of a value the model reads whole, and the boundary's decoder of it.
pub type Member = (&'static str, BoxDecoder<Value, Value>);

/// A value read whole by `model` once each of `members` it has is decoded by the boundary.
pub fn members<'a, T>(
    members: Vec<Member>,
    model: impl Decoder<Value, Output = T> + 'a,
) -> impl Decoder<Value, Output = T> + 'a {
    decoder_fn(move |given: &Value, path: &Path<'_>| {
        let mut decoded = given.clone();
        let mut found = Issues::new();
        let mut refused: Vec<Pointer> = Vec::new();
        if let Value::Object(object) = &mut decoded {
            for (name, decoder) in &members {
                let Some(member) = object.get(*name) else {
                    continue;
                };
                let at = path.key(name);
                match decoder.decode_at(member, &at) {
                    Ok(written) => {
                        object.insert((*name).to_owned(), written);
                    }
                    Err(issues) => {
                        object.remove(*name);
                        refused.push(at.to_pointer());
                        found.merge(issues);
                    }
                }
            }
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

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use raoh::Issue;

    use super::*;

    /// A model that records what it was handed, and reports each of `required` it is missing.
    fn recording<'r>(
        read: &'r RefCell<Vec<Value>>,
        required: &'r [&'r str],
    ) -> impl Decoder<Value, Output = ()> + 'r {
        decoder_fn(move |given: &Value, path: &Path<'_>| {
            read.borrow_mut().push(given.clone());
            let mut missing = Issues::new();
            for name in required {
                if given.get(*name).is_none() {
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
                .map(Value::String)
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

        assert_eq!(read.into_inner(), [json!({ "email": "a@example.com" })]);
    }

    #[test]
    fn the_model_is_never_handed_what_the_boundary_refused() {
        let read = RefCell::new(Vec::new());

        let issues = members(vec![email()], recording(&read, &["email", "city"]))
            .decode(&json!({ "email": " X ", "city": "Tokyo" }))
            .unwrap_err();

        assert_eq!(read.into_inner(), [json!({ "city": "Tokyo" })]);
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
    fn only_the_refused_members_own_path_is_taken_for_the_boundarys() {
        // The boundary's decoder of an address refuses its postcode, and the address is taken out.
        // The model reports it missing, which is dropped, and the model's own issue beside it is
        // kept: nothing at a path merely under or beside the refused member is dropped.
        let address = decoder_fn(|_: &Value, path: &Path<'_>| -> Result<Value, Issues> {
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
