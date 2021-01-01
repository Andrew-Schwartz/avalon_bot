macro_rules! set {
    () => { std::collections::HashSet::new() };
    ($($x:expr),+ $(,)?) => {{
        let mut l = std::collections::HashSet::new();
        $(
            l.insert($x);
        )+
        l
    }};
}

macro_rules! map {
    () => { std::collections::HashMap::new() };
    ($($k:expr => $v:expr),+ $(,)?) => {{
        let mut map = std::collections::HashMap::new();
        $(
            map.insert($k, $v);
        )+
        map
    }}
}