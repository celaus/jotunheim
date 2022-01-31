pub fn e_<E: Into<anyhow::Error>>(err: E) -> anyhow::Error {
    err.into()
}

pub fn avg(v: &[f64]) -> f64 {
    v.iter().fold(0_f64, |p, c| p + c) / v.len() as f64
}

pub fn max(v: &[f64]) -> f64 {
    v.iter().fold(0_f64, |p, c| p.max(*c))
}

#[macro_export]
macro_rules! extract_from {
    ( $coll: expr, $cls: path ) => {{
        $coll
            .filter_map(|r| match r {
                $cls(v) => Some(*v),
                _ => None,
            })
            .collect::<Vec<_>>()
    }};
}
