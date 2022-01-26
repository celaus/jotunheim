pub fn e_<E: Into<anyhow::Error>>(err: E) -> anyhow::Error {
    err.into()
}
