use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct PaginatedData<T> {
    pub data: Vec<T>,
    pub count: i64,
}