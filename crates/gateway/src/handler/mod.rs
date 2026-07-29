mod json_or_form;
pub mod qa;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Reply<T> {
    pub code: i64,
    pub msg: String,
    pub data: Option<T>,
}
