use std::sync::{Arc, Mutex};

use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request};
use url::Url;

use super::{BrokerNet, FetchOutcome, GrpcAsk, GrpcCallOutcome, ResourceLog};
use crate::broker::{Allowance, Broker, Channel, Fetch};
use crate::budget::Budget;
use crate::receipt::RequestRecord;
use crate::secrets::{Resolved, Secrets};

struct SuppliedBroker {
    outcome: FetchOutcome,
    requests: Mutex<Vec<Fetch>>,
    budget: Budget,
}

impl SuppliedBroker {
    fn new(outcome: FetchOutcome) -> Self {
        Self {
            outcome,
            requests: Mutex::default(),
            budget: Budget::default(),
        }
    }
}

impl Broker for SuppliedBroker {
    fn send(&self, fetch: &Fetch) -> FetchOutcome {
        self.requests.lock().unwrap().push(fetch.clone());
        self.outcome.clone()
    }

    fn records(&self) -> Vec<RequestRecord> {
        Vec::new()
    }
    fn budget(&self) -> Allowance {
        Allowance {
            spent: self.budget.spent(),
            limits: self.budget.limits().clone(),
        }
    }
    fn reset_budget(&self) {
        self.budget.reset();
    }
    fn cookie_count(&self) -> usize {
        0
    }
    fn document_cookie(&self, _url: &Url) -> String {
        String::new()
    }
    fn store_cookie(&self, _url: &Url, _header: &str) -> usize {
        0
    }
    fn keep_only_origin(&self, _origin: &Url) -> bool {
        false
    }
    fn open_socket(&self, _url: &Url, _document: Option<&Url>) -> Result<Arc<dyn Channel>, String> {
        Err("this test broker has no socket transport".into())
    }
    fn grpc_call(&self, _ask: GrpcAsk) -> GrpcCallOutcome {
        GrpcCallOutcome {
            denied: Some("this test broker has no gRPC transport".into()),
            ..Default::default()
        }
    }
    fn open_event_stream(
        &self,
        _url: &Url,
        _document: Option<&Url>,
    ) -> Result<Arc<dyn Channel>, String> {
        Err("this test broker has no event transport".into())
    }
    #[cfg(feature = "identity")]
    fn identity(&self) -> Arc<crate::identity::Identity> {
        Arc::new(crate::identity::native())
    }
    fn secret_names(&self) -> Vec<String> {
        Vec::new()
    }
    fn substitute(&self, text: &str) -> Resolved {
        Secrets::default().substitute(text)
    }
    fn redact(&self, text: &str) -> String {
        text.into()
    }
    fn has_redactions(&self) -> bool {
        false
    }
}

#[derive(Default)]
struct Delivery(Mutex<Option<(String, Vec<u8>)>>);

struct DeliveryHandler(Arc<Delivery>);

impl NetHandler for DeliveryHandler {
    fn bytes(self: Box<Self>, url: String, bytes: Bytes) {
        *self.0.0.lock().unwrap() = Some((url, bytes.to_vec()));
    }
}

fn url(path: &str) -> Url {
    Url::parse(path).unwrap()
}

fn success(body: &[u8]) -> FetchOutcome {
    FetchOutcome {
        seq: Some(1),
        headers: vec![("Content-Type".into(), "text/html; charset=utf-8".into())],
        final_url: url("https://tasks.test/result"),
        body: body.into(),
        status: Some(200),
        error: None,
        opaque: false,
    }
}

#[test]
fn supplied_broker_receives_document_origin_and_returns_resource_bytes() {
    let broker = Arc::new(SuppliedBroker::new(success(b"body")));
    let resources = ResourceLog::default();
    let base = url("https://tasks.test/catalog");
    let requested = url("https://tasks.test/style.css");
    let net = BrokerNet::with_log(broker.clone(), Some(base.clone()), resources.clone());
    let delivery = Arc::new(Delivery::default());
    net.fetch(
        0,
        Request::get(requested.clone()),
        Box::new(DeliveryHandler(delivery.clone())),
    );
    assert_eq!(broker.requests.lock().unwrap()[0].document, Some(base));
    assert_eq!(
        *delivery.0.lock().unwrap(),
        Some(("https://tasks.test/result".into(), b"body".to_vec()))
    );
    let resources = resources.lock().unwrap();
    assert_eq!(resources.status(requested.as_str()), Some(200));
    assert_eq!(resources.status("https://tasks.test/result"), Some(200));
}

#[test]
fn refused_resources_complete_without_reaching_another_transport() {
    let requested = url("https://tasks.test/denied");
    let broker = Arc::new(SuppliedBroker::new(FetchOutcome::refused(
        requested.clone(),
        "denied".into(),
    )));
    let resources = ResourceLog::default();
    let net = BrokerNet::with_log(broker.clone(), None, resources.clone());
    let delivery = Arc::new(Delivery::default());
    net.fetch(
        0,
        Request::get(requested.clone()),
        Box::new(DeliveryHandler(delivery.clone())),
    );
    assert_eq!(broker.requests.lock().unwrap().len(), 1);
    assert_eq!(
        *delivery.0.lock().unwrap(),
        Some((requested.to_string(), Vec::new()))
    );
    assert_eq!(
        resources.lock().unwrap().status(requested.as_str()),
        Some(0)
    );
}

#[test]
fn static_forms_use_the_supplied_broker_without_a_native_client() {
    let broker = Arc::new(SuppliedBroker::new(success(b"<title>Accepted</title>")));
    let factory =
        crate::PageFactory::new(broker.clone(), Vec::new(), crate::PageOptions::default());
    let mut page = factory.from_html(
        "<title>Catalog</title><form method='post' action='/answer'>\
         <input name='answer' placeholder='Amount'><button>Save</button></form>",
        &url("https://tasks.test/catalog"),
    );
    let snapshot = page.snapshot();
    let field = snapshot
        .refs
        .iter()
        .find(|entry| entry.name == "Amount")
        .unwrap()
        .node_id;
    let button = snapshot
        .refs
        .iter()
        .find(|entry| entry.name == "Save")
        .unwrap()
        .node_id;
    assert!(!page.type_into(button, "42"));
    assert!(page.type_into(field, "42"));
    let submission = page.submit_form(button).unwrap();
    assert_eq!(submission.body, b"answer=42");
    let next = factory.open_submission(&submission).unwrap();
    assert_eq!(next.snapshot().title, "Accepted");
    let requests = broker.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.as_str(), "https://tasks.test/answer");
    assert_eq!(requests[0].body, b"answer=42");
    assert_eq!(
        requests[0].document,
        Some(url("https://tasks.test/catalog"))
    );
}

#[test]
fn a_supplied_broker_failure_is_returned_to_the_page_caller() {
    let broker = Arc::new(SuppliedBroker::new(FetchOutcome::refused(
        url("https://tasks.test/denied"),
        "fixture denied".into(),
    )));
    let factory = crate::PageFactory::new(broker, Vec::new(), crate::PageOptions::default());
    let error = match factory.open(&url("https://tasks.test/denied")) {
        Ok(_) => panic!("a refused request must not produce a page"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("fixture denied"));
}
