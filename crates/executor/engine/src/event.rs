use uuid::Uuid;

#[derive(Debug, Clone)]
#[must_use]
pub enum Event {
    Invoke { remote: Uuid, payload: Vec<u8> },
    SetState { remote: Uuid, payload: Vec<u8> },
    GetState { remote: Uuid, payload: Vec<u8> },
    Finish { remote: Uuid, payload: Vec<u8> },
}
