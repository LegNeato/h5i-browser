//! CSS queries shared by extraction, action selection, and JavaScript.

/// Every node under `scope` matching `selector`, in document order.
pub(crate) fn matches_within(
    doc: &blitz_dom::BaseDocument,
    scope: usize,
    selector: &str,
) -> Vec<usize> {
    let Ok(list) = doc.try_parse_selector_list(selector) else {
        return Vec::new();
    };

    if scope == 0 {
        let mut found = smallvec::SmallVec::<[&blitz_dom::Node; 128]>::new();
        style::dom_apis::query_selector::<&blitz_dom::Node, style::dom_apis::QueryAll>(
            doc.root_node(),
            &list,
            &mut found,
            style::dom_apis::MayUseInvalidation::Yes,
        );
        return found.into_iter().map(|node| node.id).collect();
    }

    let mut out = Vec::new();
    collect_matches(doc, scope, &list, &mut out);
    out
}

/// The first match in document order, stopping there.
///
/// The same matcher and the same selector list as [`matches_within`], asked a
/// narrower question. Stylo's `QueryFirst` sets
/// `should_stop_after_first_match`, so a selector whose target is early in the
/// document costs a fraction of the full walk; one whose target is last costs
/// the same. Document-scoped only, which is the shape every caller needs.
pub(crate) fn first_match_in_document(
    doc: &blitz_dom::BaseDocument,
    selector: &str,
) -> Option<usize> {
    let list = doc.try_parse_selector_list(selector).ok()?;
    let mut found: Option<&blitz_dom::Node> = None;
    style::dom_apis::query_selector::<&blitz_dom::Node, style::dom_apis::QueryFirst>(
        doc.root_node(),
        &list,
        &mut found,
        style::dom_apis::MayUseInvalidation::Yes,
    );
    found.map(|node| node.id)
}

/// Depth-first over the descendants of `id`, in document order.
///
/// The scope itself is deliberately not tested: `element.querySelector` is
/// defined to search inside the element, not to return it.
fn collect_matches(
    doc: &blitz_dom::BaseDocument,
    id: usize,
    list: &selectors::SelectorList<style::selector_parser::SelectorImpl>,
    out: &mut Vec<usize>,
) {
    let Some(node) = doc.get_node(id) else { return };
    for child in node.children.clone() {
        if let Some(child_node) = doc.get_node(child)
            && child_node.is_element()
                // Standards mode: this engine parses HTML5 and never emulates
                // the quirks the flag exists to preserve.
                && style::dom_apis::element_matches(
                    &child_node,
                    list,
                    style::context::QuirksMode::NoQuirks,
                )
        {
            out.push(child);
        }
        collect_matches(doc, child, list, out);
    }
}
