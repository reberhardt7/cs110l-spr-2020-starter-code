use async_trait::async_trait;

#[async_trait]
pub trait Server {
    async fn stop(self: Box<Self>) -> usize;
    fn address(&self) -> String;
}
