use serde::de::DeserializeOwned;

/// 1件ずつデシリアライズした一覧。
///
/// 壊れた要素は落とすが、落とした事実は `skipped` に残す。黙って落とすと
/// スキーマが変わったときにデータが静かに消え、誰も気づかない。
#[derive(Debug, Clone)]
pub struct PartialList<T> {
    /// 読めた要素。**元の応答の順序を保つ。**
    pub items: Vec<T>,
    /// 読めなかった要素。正常時は空。
    pub skipped: Vec<SkippedItem>,
}

/// デシリアライズできなかった要素。
///
/// 中身(生のJSON)は持たない。任意の内容が下流の出力に混ざり、量も
/// 見積もれないため。中身が要るときは同じ端点を直接叩いて確認する。
#[derive(Debug, Clone)]
pub struct SkippedItem {
    /// 応答配列の何番目か(0始まり)。
    pub index: usize,
    /// 生のJSONから拾った `id`。
    ///
    /// 数値でも文字列でも文字列として持つ。課題のコメントIDは数値、
    /// ドキュメントのコメントIDは文字列で来る。
    pub id: Option<String>,
    /// serde のエラーメッセージ。
    pub reason: String,
}

impl<T: DeserializeOwned> PartialList<T> {
    /// JSONの配列を1件ずつデシリアライズする。
    pub(crate) fn from_values(values: Vec<serde_json::Value>) -> Self {
        let mut items = Vec::with_capacity(values.len());
        let mut skipped = Vec::new();
        for (index, value) in values.into_iter().enumerate() {
            let id = scalar_id(&value);
            match serde_json::from_value::<T>(value) {
                Ok(item) => items.push(item),
                Err(e) => skipped.push(SkippedItem {
                    index,
                    id,
                    reason: e.to_string(),
                }),
            }
        }
        Self { items, skipped }
    }
}

/// 生のJSONから `id` を文字列として拾う。オブジェクトでない、`id` が無い、
/// 数値でも文字列でもない場合は `None`。
fn scalar_id(value: &serde_json::Value) -> Option<String> {
    match value.get("id")? {
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Item {
        id: u64,
        name: String,
    }

    fn values(raw: &str) -> Vec<serde_json::Value> {
        serde_json::from_str(raw).unwrap()
    }

    /// 全件正常なら items に全部入り、skipped は空。
    #[test]
    fn keeps_every_item_when_all_are_valid() {
        let list = PartialList::<Item>::from_values(values(
            r#"[{"id":1,"name":"a"},{"id":2,"name":"b"}]"#,
        ));
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].id, 1);
        assert_eq!(list.items[0].name, "a");
        assert_eq!(list.items[1].id, 2);
        assert!(list.skipped.is_empty());
    }

    /// 壊れた要素を落としても、**残りの順序が保たれる**こと。
    ///
    /// 通知は新しい順で返るので、順序が崩れると表示順が壊れる。
    #[test]
    fn drops_the_bad_item_and_preserves_the_order_of_the_rest() {
        let list = PartialList::<Item>::from_values(values(
            r#"[{"id":1,"name":"a"},{"id":2},{"id":3,"name":"c"}]"#,
        ));
        assert_eq!(
            list.items.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert_eq!(list.items[0].name, "a");
        assert_eq!(list.items[1].name, "c");
        assert_eq!(list.skipped.len(), 1);
        // index は**元の配列での位置**であって、items 側の位置ではない
        assert_eq!(list.skipped[0].index, 1);
        assert_eq!(list.skipped[0].id.as_deref(), Some("2"));
        assert!(
            list.skipped[0].reason.contains("name"),
            "serde のメッセージが入っていない: {}",
            list.skipped[0].reason
        );
    }

    /// `id` が文字列でも拾えること。
    #[test]
    fn reads_a_string_id() {
        let list = PartialList::<Item>::from_values(values(r#"[{"id":"doc-1"}]"#));
        assert_eq!(list.skipped[0].id.as_deref(), Some("doc-1"));
    }

    /// `id` が無い・オブジェクトでない・数値でも文字列でもない場合は None。
    #[test]
    fn leaves_the_id_empty_when_it_cannot_be_read() {
        let list = PartialList::<Item>::from_values(values(
            r#"[{"name":"no id"}, 42, {"id":{"nested":1}}]"#,
        ));
        assert_eq!(list.skipped.len(), 3);
        for s in &list.skipped {
            assert!(s.id.is_none(), "id が拾えてしまっている: {:?}", s);
        }
        assert_eq!(
            list.skipped.iter().map(|s| s.index).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    /// **全件壊れていてもエラーにはしない。**
    ///
    /// 「配列かどうか」だけが失敗の境界であり、中身が全部読めないことは
    /// 空の結果として扱う。
    #[test]
    fn returns_an_empty_list_rather_than_failing_when_every_item_is_bad() {
        let list = PartialList::<Item>::from_values(values(r#"[{"x":1},{"y":2}]"#));
        assert!(list.items.is_empty());
        assert_eq!(list.skipped.len(), 2);
    }

    /// 空配列は空の結果。
    #[test]
    fn handles_an_empty_array() {
        let list = PartialList::<Item>::from_values(values("[]"));
        assert!(list.items.is_empty());
        assert!(list.skipped.is_empty());
    }
}
