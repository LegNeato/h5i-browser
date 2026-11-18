use super::*;

fn page_and_script(html: &str) -> (crate::engine::Page, Script) {
    let broker =
        crate::net::LocalBroker::new(Policy::new(), Arc::new(MemorySink::new()), None).unwrap();
    let factory = PageFactory::new(broker, Vec::new(), PageOptions::default());
    let base = url::Url::parse("https://app.example/").unwrap();
    let page = factory.from_html(html, &base);
    let script = Script::new(page.dom(), factory.broker().clone(), &base).unwrap();
    (page, script)
}

#[test]
fn assignments_replace_the_editor_without_changing_defaults_or_firing_events() {
    let (page, mut script) = page_and_script("<input id='field' value='original'>");
    let node = page
        .dom()
        .borrow()
        .query_selector("#field")
        .unwrap()
        .unwrap();
    script
        .eval(
            "const field = document.getElementById('field'); \
             let events = 0; \
             field.addEventListener('input', () => events++); \
             field.addEventListener('change', () => events++);",
        )
        .unwrap();

    for value in ["replacement", "short", "", "  spaced  ", " \t "] {
        let literal = serde_json::to_string(value).unwrap();
        script.eval(&format!("field.value = {literal}")).unwrap();
        assert_eq!(script.eval_value("field.value").unwrap(), value);
        assert_eq!(page.field_value(node).as_deref(), Some(value));
        assert_eq!(script.eval_value("field.defaultValue").unwrap(), "original");
        assert_eq!(
            script.eval_value("field.getAttribute('value')").unwrap(),
            "original"
        );
    }
    assert_eq!(script.eval_value("events").unwrap(), "0");
}

#[test]
fn range_replacement_updates_the_value_and_selection() {
    let (_page, mut script) = page_and_script("<input id='field' value='hello world'>");
    script
        .eval(
            "const field = document.getElementById('field'); \
             field.setSelectionRange(0, 5); \
             field.setRangeText('HI'); \
             field.select();",
        )
        .unwrap();
    assert_eq!(script.eval_value("field.value").unwrap(), "HI world");
    assert_eq!(script.eval_value("field.selectionStart").unwrap(), "0");
    assert_eq!(script.eval_value("field.selectionEnd").unwrap(), "8");
}

#[test]
fn numeric_steps_replace_the_previous_value() {
    let (_page, mut script) =
        page_and_script("<input id='field' type='number' value='7' step='2'>");
    script
        .eval("const field = document.getElementById('field'); field.stepUp();")
        .unwrap();
    assert_eq!(script.eval_value("field.valueAsNumber").unwrap(), "9");
    script.eval("field.stepDown(2)").unwrap();
    assert_eq!(script.eval_value("field.valueAsNumber").unwrap(), "5");
    assert_eq!(
        script.eval_value("String(field.selectionStart)").unwrap(),
        "null"
    );
}

#[test]
fn detached_values_survive_and_file_inputs_still_reject_assignments() {
    let (_page, mut script) = page_and_script("<input id='file' type='file'>");
    script
        .eval(
            "const detached = document.createElement('input'); \
             detached.value = 'first'; detached.value = 'second'; \
             let rejected = false; \
             try { document.getElementById('file').value = 'secret.txt'; } \
             catch (error) { rejected = error.name === 'InvalidStateError'; }",
        )
        .unwrap();
    assert_eq!(script.eval_value("detached.value").unwrap(), "second");
    assert_eq!(script.eval_value("rejected").unwrap(), "true");
    assert_eq!(
        script
            .eval_value("document.getElementById('file').value")
            .unwrap(),
        ""
    );
}
