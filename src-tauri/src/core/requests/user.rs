use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    pub id: String,
    pub email: String,
    pub nickname: String,
    pub status: String,
    pub avatar: String,
    pub created_at: String,
    pub group: Group,
    pined: Vec<Pined>,
    language: String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub permission: String,
    pub direct_link_batch_size: i64,
    pub trash_retention: i64,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Pined {
    pub uri: String,
}
