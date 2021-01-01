use crate::cache::Cache;

#[async_trait::async_trait]
pub trait Update {
    async fn update(self, cache: &Cache);
}