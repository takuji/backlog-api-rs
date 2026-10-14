/// クエリ文字列・フォームボディ用のkey-valueペア列。
///
/// Backlog APIの配列パラメータは `projectId[]=1&projectId[]=2` 形式のため、
/// serde_urlencoded ではなく自前でペアを組み立てる。
#[derive(Debug, Clone, Default)]
pub(crate) struct QueryPairs(Vec<(String, String)>);

impl QueryPairs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, key: &str, value: impl ToString) {
        self.0.push((key.to_string(), value.to_string()));
    }

    /// keyは `"projectId[]"` のように `[]` サフィックス込みで渡す。
    pub fn push_array<I, T>(&mut self, key: &str, values: I)
    where
        I: IntoIterator<Item = T>,
        T: ToString,
    {
        for v in values {
            self.push(key, v);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_slice(&self) -> &[(String, String)] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_push_array_build_pairs() {
        let mut pairs = QueryPairs::new();
        pairs.push("keyword", "bug");
        pairs.push("count", 50);
        pairs.push_array("projectId[]", [1u64, 2]);
        assert_eq!(
            pairs.as_slice(),
            &[
                ("keyword".to_string(), "bug".to_string()),
                ("count".to_string(), "50".to_string()),
                ("projectId[]".to_string(), "1".to_string()),
                ("projectId[]".to_string(), "2".to_string()),
            ]
        );
    }

    #[test]
    fn new_is_empty() {
        assert!(QueryPairs::new().is_empty());
    }
}
