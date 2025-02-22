use std::str::FromStr;

pub struct PostgresUri(String);

impl FromStr for PostgresUri {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}