use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
/*/
CREATE TABLE users_votes (
    id bigserial PRIMARY KEY,
    target_id bigint NOT NULL DEFAULT 0,
    target_type varchar(50) NOT NULL DEFAULT '',
    created_by varchar(50) NOT NULL DEFAULT '',
    created_at timestamp NOT NULL
);
*/
use sqlx::prelude::FromRow;

const USER_VOTE_TABLE: &str = "users_votes";

#[derive(Default, Debug, FromRow, Serialize, Deserialize)]
pub struct UserVoteEntity {
    pub id: i64,
    pub target_id: i64,
    pub target_type: String,
    pub created_by: String,
    pub created_at: NaiveDateTime,
}
impl UserVoteEntity {
    pub fn table_name() -> String {
        USER_VOTE_TABLE.to_string()
    }
}


