#[derive(Debug, Clone)]
pub enum Select<L, R> {
    Left(L),
    Right(R),
}
