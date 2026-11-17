//! Namespace declarations (Recommendation section 3.7.4).

use std::collections::HashSet;

use super::{NameParts, Parser, at};
use crate::error::clip;
use crate::pn;
use crate::provn::Relaxation;
use crate::{Error, Namespaces, Result, namespaces};

impl<'t> Parser<'t> {
    // ANCHOR: namespace_declarations
    // [45] namespaceDeclarations ::=
    //     (defaultNamespaceDeclaration | namespaceDeclaration) (namespaceDeclaration)*
    pub(super) fn namespace_declarations(&mut self) -> Result<Namespaces> {
        let mut ns = Namespaces::new();
        let mut seen: HashSet<&str> = HashSet::new();
        loop {
            if self.is_keyword("default") {
                if ns.default_namespace().is_some() {
                    return Err(Self::expected_found(
                        self.peek(),
                        "a namespace declaration or a statement",
                        "a second 'default' in one scope",
                    ));
                }
                // `default` comes before every `prefix`: production [45].
                if !seen.is_empty() && !self.profile.allows(Relaxation::DefaultAfterPrefix) {
                    return Err(Self::expected_found(
                        self.peek(),
                        "'default' before every 'prefix' under the strict profile",
                        "'default' after a prefix",
                    ));
                }
                self.bump();
                let token = self.peek();
                let iri = self.iri()?;
                ns.set_default(iri).map_err(|e| at(token, e))?;
            } else if self.is_keyword("prefix") {
                self.bump();
                let name = self.prefix_name()?;
                if seen.contains(name.local) {
                    return Err(Self::expected_found(
                        name.token,
                        "a new prefix",
                        &format!("'{}' declared twice in one scope", clip(name.local)),
                    ));
                }
                let token = self.peek();
                let iri = self.iri()?;
                if namespaces::is_blank(iri) {
                    let prefix = Some(name.local.to_owned());
                    return Err(at(token, Error::EmptyNamespace { prefix }));
                }
                // `prov` and `xsd` are predeclared: section 3.7.4.
                let predeclared = Namespaces::is_predeclared(name.local, iri);
                if predeclared && !self.profile.allows(Relaxation::PredeclaredPrefix) {
                    let prefix = clip(name.local);
                    return Err(Self::expected_found(
                        name.token,
                        &format!(
                            "a prefix other than the predeclared '{prefix}' under the strict profile"
                        ),
                        &format!("'prefix {prefix} <{iri}>'"),
                    ));
                }
                ns.declare(name.local, iri).map_err(|e| at(name.token, e))?;
                seen.insert(name.local);
            } else {
                return Ok(ns);
            }
        }
    }
    // ANCHOR_END: namespace_declarations

    /// The prefix in a `prefix` declaration. Production [46] takes a
    /// `PN_PREFIX`, which has no escapes, so the text as written is tested,
    /// not the unescaped name.
    fn prefix_name(&mut self) -> Result<NameParts<'t>> {
        let name = self.name("a prefix")?;
        let written = self.text_of(name.token);
        if pn::is_pn_prefix(written) {
            Ok(name)
        } else {
            Err(Self::expected_found(
                name.token,
                "a prefix",
                &format!("'{}'", clip(written)),
            ))
        }
    }
}
