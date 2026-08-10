use crate::model::enums::SessionStatus;

#[derive(Debug, Clone, PartialEq)]
pub struct Provider {
    pub id: i32,
    pub name: String,
    pub status: SessionStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_construction() {
        let p = Provider {
            id: 1,
            name: "LongPort".to_string(),
            status: SessionStatus::Connected,
        };
        assert_eq!(p.id, 1);
        assert_eq!(p.name, "LongPort");
        assert_eq!(p.status, SessionStatus::Connected);
    }
}
