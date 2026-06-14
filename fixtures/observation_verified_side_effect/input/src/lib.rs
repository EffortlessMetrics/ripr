use std::cell::RefCell;

pub struct Notifier {
    pub sent: RefCell<Vec<String>>,
}

impl Notifier {
    pub fn new() -> Self {
        Notifier { sent: RefCell::new(Vec::new()) }
    }
    pub fn send(&self, payload: &str) -> bool {
        self.sent.borrow_mut().push(payload.to_string());
        true
    }
}

pub fn process_order(notifier: &Notifier, order_id: &str) -> bool {
    notifier.send(order_id)
}
