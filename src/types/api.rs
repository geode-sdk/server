use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct PaginatedData<T> {
    pub data: Vec<T>,
    pub page: u32,
    pub count: i32,
}