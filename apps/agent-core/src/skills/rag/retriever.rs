pub trait Retriever {
    fn retrieve(&self, query: &str) -> Vec<String>;
}
