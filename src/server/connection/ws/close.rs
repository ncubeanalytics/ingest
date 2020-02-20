use actix::Message;

/// Internal message sent to the WSHandler actor when it should close.
pub struct WSClose;

impl Message for WSClose {
    type Result = ();
}
