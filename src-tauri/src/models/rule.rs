use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    ProviderType,
    ConnectionName,
    ConversationType,
    ConversationName,
    SenderEmail,
    SenderDomain,
    SenderName,
    Subject,
    Body,
    MemberCount,
    Namespace,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    Is,
    IsNot,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    Matches,
    GreaterThan,
    LessThan,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Condition {
    pub field: Field,
    pub operator: Operator,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Match {
    #[default]
    All,
    Any,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Subject {
    pub provider_type: String,
    pub connection_name: String,
    pub conversation_type: String,
    pub conversation_name: String,
    pub sender_email: String,
    pub sender_name: String,
    pub subject: String,
    pub body: String,
    pub member_count: i64,
    pub namespace: String,
}

impl Subject {
    pub fn value(&self, field: &Field) -> String {
        match field {
            Field::ProviderType => self.provider_type.clone(),
            Field::ConnectionName => self.connection_name.clone(),
            Field::ConversationType => self.conversation_type.clone(),
            Field::ConversationName => self.conversation_name.clone(),
            Field::SenderEmail => self.sender_email.clone(),
            Field::SenderDomain => self.sender_email.rsplit('@').next().unwrap_or("").to_string(),
            Field::SenderName => self.sender_name.clone(),
            Field::Subject => self.subject.clone(),
            Field::Body => self.body.clone(),
            Field::MemberCount => self.member_count.to_string(),
            Field::Namespace => self.namespace.clone(),
        }
    }
}

impl Operator {
    pub fn test(&self, actual: &str, wanted: &str) -> bool {
        let raw = actual;
        let actual = actual.to_lowercase();
        let wanted_raw = wanted.trim();
        let wanted = wanted_raw.to_lowercase();
        match self {
            Operator::Is => actual == wanted,
            Operator::IsNot => actual != wanted,
            Operator::Contains => !wanted.is_empty() && actual.contains(&wanted),
            Operator::NotContains => !actual.contains(&wanted),
            Operator::StartsWith => actual.starts_with(&wanted),
            Operator::EndsWith => actual.ends_with(&wanted),
            Operator::Matches => regex::RegexBuilder::new(wanted_raw).case_insensitive(true).build().map(|re| re.is_match(raw)).unwrap_or(false),
            Operator::GreaterThan => actual.parse::<f64>().ok().zip(wanted.parse::<f64>().ok()).map(|(a, b)| a > b).unwrap_or(false),
            Operator::LessThan => actual.parse::<f64>().ok().zip(wanted.parse::<f64>().ok()).map(|(a, b)| a < b).unwrap_or(false),
        }
    }
}

impl Condition {
    pub fn holds(&self, subject: &Subject) -> bool {
        self.operator.test(&subject.value(&self.field), &self.value)
    }
}

impl Match {
    pub fn combine(&self, conditions: &[Condition], subject: &Subject) -> bool {
        if conditions.is_empty() {
            return false;
        }
        match self {
            Match::All => conditions.iter().all(|c| c.holds(subject)),
            Match::Any => conditions.iter().any(|c| c.holds(subject)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject() -> Subject {
        Subject {
            provider_type: "google".into(),
            connection_name: "Northwind".into(),
            conversation_type: "email".into(),
            conversation_name: "Deploy".into(),
            sender_email: "bot@vercel.com".into(),
            sender_name: "Vercel".into(),
            subject: "Your deploy finished".into(),
            body: "Build 42 is live".into(),
            member_count: 2,
            namespace: "northwind.example".into(),
        }
    }

    #[test]
    fn conditions_compare_case_insensitively_and_by_domain() {
        let s = subject();
        assert!(Condition { field: Field::SenderDomain, operator: Operator::Is, value: "Vercel.com".into() }.holds(&s));
        assert!(Condition { field: Field::Subject, operator: Operator::Contains, value: "DEPLOY".into() }.holds(&s));
        assert!(Condition { field: Field::Body, operator: Operator::Matches, value: r"build \d+".into() }.holds(&s));
        assert!(Condition { field: Field::MemberCount, operator: Operator::GreaterThan, value: "1".into() }.holds(&s));
        assert!(!Condition { field: Field::SenderEmail, operator: Operator::StartsWith, value: "noreply".into() }.holds(&s));
        assert!(Operator::NotContains.test("Hello", "bye"));
    }

    #[test]
    fn all_and_any_combine_conditions() {
        let s = subject();
        let hit = Condition { field: Field::ProviderType, operator: Operator::Is, value: "google".into() };
        let miss = Condition { field: Field::ProviderType, operator: Operator::Is, value: "slack".into() };
        assert!(Match::All.combine(&[hit.clone()], &s));
        assert!(!Match::All.combine(&[hit.clone(), miss.clone()], &s));
        assert!(Match::Any.combine(&[hit, miss], &s));
        assert!(!Match::Any.combine(&[], &s));
    }
}
