use anyhow::Result;
use pulsar::{DeserializeMessage, Error, Payload, SerializeMessage, producer::Message};
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize)]
pub struct VoteMessage {
    pub target_id: i64,
    pub target_type: String,
    pub created_by: String,
    pub action: String,
}

impl SerializeMessage for VoteMessage {
    fn serialize_message(input: Self) -> Result<Message, Error> {
        let data = serde_json::to_vec(&input).map_err(|e| Error::Custom(e.to_string()))?;
        let message = Message {
            payload: data,
            ..Default::default()
        };
        Ok(message)
    }
}

impl DeserializeMessage for VoteMessage {
    type Output = Result<Self, serde_json::Error>;

    fn deserialize_message(payload: &Payload) -> Self::Output {
        serde_json::from_slice(&payload.data)
    }
}
