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

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use pulsar::{DeserializeMessage, Payload, SerializeMessage};

    use super::VoteMessage;

    #[test]
    fn test_should_serialize_and_deserialize_vote_message() -> Result<()> {
        let message = VoteMessage {
            target_id: 42,
            target_type: "answer".to_string(),
            created_by: "用户甲".to_string(),
            action: "up".to_string(),
        };

        let serialized = VoteMessage::serialize_message(message)?;
        let payload = Payload {
            metadata: Default::default(),
            data: serialized.payload,
        };
        let deserialized = VoteMessage::deserialize_message(&payload)?;

        assert_eq!(deserialized.target_id, 42);
        assert_eq!(deserialized.target_type, "answer");
        assert_eq!(deserialized.created_by, "用户甲");
        assert_eq!(deserialized.action, "up");
        Ok(())
    }

    #[test]
    fn test_should_reject_malformed_vote_payload() {
        let payload = Payload {
            metadata: Default::default(),
            data: br#"{"target_id":"not-a-number"}"#.to_vec(),
        };

        assert!(VoteMessage::deserialize_message(&payload).is_err());
    }
}
