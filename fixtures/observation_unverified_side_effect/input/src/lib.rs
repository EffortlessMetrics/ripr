pub struct Notifier;

impl Notifier {
    pub fn send(&self, payload: &str) -> bool {
        let _ = payload;
        true
    }
}

pub fn process_order(notifier: &Notifier, order_id: &str) -> bool {
    notifier.send(order_id)
}
