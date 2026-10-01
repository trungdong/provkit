//! Namespace declarations (Recommendation section 3.7.4).

use std::collections::HashSet;

use super::{NameParts, Parser, at};
use crate::error::clip;
use crate::pn;
use crate::provn::Relaxation;
use crate::{Namespaces, Result};

impl<'t> Parser<'t> {
    // namespaceDeclarations ::= (defaultNamespaceDeclaration | namespaceDeclaration)*

    pub(super) fn namespace_declarations(&mut self) -> Result<Namespaces> {
        let mut ns = Namespaces::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut has_default = false;
        loop {
            if self.is_keyword("default") {
                if has_default {
                    return Err(Self::error_at(
                        self.peek(),
                        "expected a namespace declaration or a statement, found a second \
                         'default' in one scope"
                            .to_owned(),
                    ));
                }
                // `default` comes before every `prefix`: production [45].
                if !seen.is_empty() && !self.profile.allows(Relaxation::DefaultAfterPrefix) {
                    return Err(Self::error_at(
                        self.peek(),
                        "expected 'default' before every 'prefix' under the strict profile, \
                         found 'default' after a prefix"
                            .to_owned(),
                    ));
                }
                has_default = true;
                self.bump();
                let iri = self.iri()?;
                ns.set_default(&iri);
            } else if self.is_keyword("prefix") {
                self.bump();
                let name = self.prefix_name()?;
                if seen.contains(&name.local) {
                    return Err(Self::error_at(
                        name.token,
                        format!(
                            "expected a new prefix, found '{}' declared twice in one scope",
                            clip(&name.local)
                        ),
                    ));
                }
                let iri = self.iri()?;
                // `prov` and `xsd` are predeclared: section 3.7.4.
                let predeclared = Namespaces::is_predeclared(&name.local, &iri);
                if predeclared && !self.profile.allows(Relaxation::PredeclaredPrefix) {
                    return Err(Self::error_at(
                        name.token,
                        format!(
                            "expected a prefix other than the predeclared '{}' under the \
                             strict profile, found 'prefix {} <{iri}>'",
                            clip(&name.local),
                            clip(&name.local)
                        ),
                    ));
                }
                ns.declare(&name.local, &iri)
                    .map_err(|e| at(name.token, e))?;
                seen.insert(name.local);
            } else {
                return Ok(ns);
            }
        }
    }

    /// The prefix in a `prefix` declaration. Production [46] takes a
    /// `PN_PREFIX`, which has no escapes, so the text as written is tested,
    /// not the unescaped name.
    fn prefix_name(&mut self) -> Result<NameParts<'t>> {
        let name = self.name("a prefix")?;
        let written = self.text_of(name.token);
        if pn::is_pn_prefix(written) {
            Ok(name)
        } else {
            Err(Self::error_at(
                name.token,
                format!("expected a prefix, found '{}'", clip(written)),
            ))
        }
    }
}
