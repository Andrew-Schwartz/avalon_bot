use crate::cache::Cache;

// BIG OL TODO: take self by ref, that way we only clone when necessary in `update`
#[async_trait::async_trait]
pub trait Update {
    async fn update(self, cache: &Cache);
}